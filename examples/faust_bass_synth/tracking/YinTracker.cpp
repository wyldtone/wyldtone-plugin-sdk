// SPDX-License-Identifier: Apache-2.0
#include "YinTracker.h"

#include <algorithm>
#include <cmath>

namespace wyldtone {
namespace {
constexpr float kPi = 3.14159265358979323846f;
float onepole(float freq, float sample_rate) noexcept {
    return 1.0f - std::exp(-2.0f * kPi * freq / sample_rate);
}
float cents_apart(float left, float right) noexcept {
    if (left <= 0.0f || right <= 0.0f) return 100000.0f;
    return std::abs(1200.0f * std::log2(left / right));
}
} // namespace

YinTracker::YinTracker(float sample_rate) { prepare(sample_rate); }

void YinTracker::prepare(float sample_rate) noexcept {
    sample_rate_ = std::clamp(sample_rate, 8000.0f, 192000.0f);
    decimate_factor_ = std::max(1, static_cast<int>(std::lround(sample_rate_ / 6000.0f)));
    reduced_sample_rate_ = sample_rate_ / static_cast<float>(decimate_factor_);
    min_lag_ = std::max(2, static_cast<int>(std::floor(reduced_sample_rate_ / kMaxPitchHz)));
    max_lag_ = std::min(kMaxLag, static_cast<int>(std::ceil(reduced_sample_rate_ / kMinPitchHz)));
    highpass_alpha_ = onepole(15.0f, sample_rate_);
    lowpass_alpha_ = onepole(480.0f, sample_rate_);
    env_attack_alpha_ = onepole(40.0f, sample_rate_);
    env_release_alpha_ = onepole(10.0f, sample_rate_);
    novelty_fast_alpha_ = onepole(28.0f, reduced_sample_rate_);
    novelty_slow_alpha_ = onepole(1.8f, reduced_sample_rate_);
    analysis_hop_ = std::max(1, static_cast<int>(std::lround(reduced_sample_rate_ * 0.004f)));
    gate_release_samples_ = static_cast<int>(sample_rate_ * 0.060f);
    onset_cooldown_samples_ = static_cast<int>(sample_rate_ * 0.095f);
    reacquire_timeout_samples_ = static_cast<int>(sample_rate_ * 0.23f);
    reset();
}

void YinTracker::set_tracking_response_ms(float milliseconds) noexcept {
    if (!std::isfinite(milliseconds)) return;
    // Low values favor faster switching; high values require more observations.
    reacquire_confirmations_ = milliseconds < 45.0f ? 1 : milliseconds < 80.0f ? 2 : 3;
}

void YinTracker::reset() noexcept {
    ring_.fill(0.0f);
    residual_ring_.fill(0.0f);
    cmnd_.fill(0.0f);
    ring_head_ = ring_count_ = 0;
    decimate_counter_ = since_analysis_ = gate_quiet_samples_ = 0;
    onset_samples_ = onset_reduced_samples_ = 0;
    pitch_lock_frames_ = 0;
    dc_ = lp_a_ = lp_b_ = lp_c_ = lp_d_ = envelope_ = 0.0f;
    novelty_fast_ = novelty_slow_ = signal_fast_ = 0.0f;
    pending_hz_ = reacquire_pending_hz_ = comb_period_samples_ = 0.0f;
    pending_count_ = reacquire_pending_count_ = 0;
    onset_armed_ = reacquiring_ = false;
    output_ = {};
}

float YinTracker::sample_from_latest(const std::array<float, kRingSize>& ring, int delay) const noexcept {
    int index = ring_head_ - 1 - delay;
    if (index < 0) index += kRingSize;
    return ring[static_cast<std::size_t>(index)];
}

void YinTracker::start_reacquisition() noexcept {
    if (!output_.valid || output_.hz <= 0.0f || ring_count_ < kDiffWindow + max_lag_ + 1)
        return;
    reacquiring_ = true;
    comb_period_samples_ = reduced_sample_rate_ / output_.hz;
    onset_samples_ = onset_reduced_samples_ = 0;
    reacquire_pending_hz_ = 0.0f;
    reacquire_pending_count_ = 0;
    // Reset the residual history; ordinary YIN history and accepted note survive.
    residual_ring_.fill(0.0f);
}

void YinTracker::push_downsampled(float x) noexcept {
    float residual = 0.0f;
    const float period = reacquiring_ ? comb_period_samples_ :
        (output_.valid && output_.hz > 0.0f ? reduced_sample_rate_ / output_.hz : 0.0f);
    if (period >= 2.0f && ring_count_ > max_lag_ + 2) {
        const int lag = static_cast<int>(period);
        const float fraction = period - static_cast<float>(lag);
        // ring still contains x[t-1], so delay=lag-1 yields x[t-lag].
        const float previous = sample_from_latest(ring_, lag - 1) * (1.0f - fraction)
                             + sample_from_latest(ring_, lag) * fraction;
        residual = x - previous;
    }
    // Periodic old-string content cancels in x[t] - x[t-T_old]. A new
    // pluck leaves a strong residual even when the previous note is louder.
    // This is an onset/novelty detector, NOT a polyphonic pitch detector.
    const float delta = std::abs(residual);
    novelty_fast_ += novelty_fast_alpha_ * (delta - novelty_fast_);
    novelty_slow_ += novelty_slow_alpha_ * (delta - novelty_slow_);
    signal_fast_ += novelty_fast_alpha_ * (std::abs(x) - signal_fast_);
    const float relative_novelty = novelty_fast_ / std::max(signal_fast_, 0.001f);
    if (output_.valid && !reacquiring_ && onset_samples_ >= onset_cooldown_samples_)
        onset_armed_ = true;
    if (output_.valid && onset_armed_ && !reacquiring_ &&
        onset_samples_ >= onset_cooldown_samples_ &&
        novelty_fast_ > 0.004f && relative_novelty > 0.37f &&
        novelty_fast_ > 1.9f * novelty_slow_) {
        start_reacquisition();
        onset_armed_ = false;
    }
    ring_[static_cast<std::size_t>(ring_head_)] = x;
    residual_ring_[static_cast<std::size_t>(ring_head_)] = residual;
    ring_head_ = (ring_head_ + 1) % kRingSize;
    if (ring_count_ < kRingSize) ++ring_count_;
    if (reacquiring_) ++onset_reduced_samples_;
    if (++since_analysis_ >= analysis_hop_) {
        since_analysis_ = 0;
        if (ring_count_ >= kDiffWindow + max_lag_ + 1) analyze();
    }
}

YinTracker::Candidate YinTracker::analyze_buffer(const std::array<float, kRingSize>& ring,
                                                 int window) noexcept {
    float sum_difference = 0.0f;
    int best_tau = 0;
    float best_cmnd = 1.0f;
    cmnd_[0] = 1.0f;
    for (int tau = 1; tau <= max_lag_; ++tau) {
        float d = 0.0f;
        for (int j = 0; j < window; ++j) {
            const float delta = sample_from_latest(ring, j) - sample_from_latest(ring, j + tau);
            d += delta * delta;
        }
        sum_difference += d;
        const float normalized = sum_difference > 1e-10f
            ? d * static_cast<float>(tau) / sum_difference : 1.0f;
        cmnd_[static_cast<std::size_t>(tau)] = normalized;
        if (tau >= min_lag_ && normalized < best_cmnd) {
            best_cmnd = normalized;
            best_tau = tau;
        }
    }
    constexpr float kYinThreshold = 0.12f;
    int selected = 0;
    for (int tau = min_lag_; tau < max_lag_; ++tau) {
        if (cmnd_[static_cast<std::size_t>(tau)] < kYinThreshold &&
            cmnd_[static_cast<std::size_t>(tau)] <= cmnd_[static_cast<std::size_t>(tau + 1)]) {
            selected = tau;
            break;
        }
    }
    if (!selected && best_tau && best_cmnd <= 0.20f) selected = best_tau;
    if (!selected) return {};

    const float confidence = std::clamp(1.0f - cmnd_[static_cast<std::size_t>(selected)], 0.0f, 1.0f);
    const int previous = std::max(1, selected - 1);
    const int following = std::min(max_lag_, selected + 1);
    const float y0 = cmnd_[static_cast<std::size_t>(previous)];
    const float y1 = cmnd_[static_cast<std::size_t>(selected)];
    const float y2 = cmnd_[static_cast<std::size_t>(following)];
    const float denominator = y0 - 2.0f * y1 + y2;
    float offset = 0.0f;
    if (std::abs(denominator) > 1e-7f)
        offset = std::clamp(0.5f * (y0-y2) / denominator, -0.5f, 0.5f);
    const float hz = reduced_sample_rate_ / (static_cast<float>(selected) + offset);
    if (!std::isfinite(hz) || hz < kMinPitchHz || hz > kMaxPitchHz || confidence < 0.80f)
        return {};
    return {hz, confidence, true};
}

void YinTracker::accept_pitch(Candidate candidate, bool reacquired) noexcept {
    float& pending_hz = reacquired ? reacquire_pending_hz_ : pending_hz_;
    int& pending_count = reacquired ? reacquire_pending_count_ : pending_count_;
    if (cents_apart(candidate.hz, pending_hz) < 55.0f) ++pending_count;
    else pending_count = 1;
    pending_hz = candidate.hz;
    if (pending_count >= (reacquired ? reacquire_confirmations_ : 2)) {
        output_.hz = candidate.hz;
        output_.confidence = candidate.confidence;
        output_.valid = true;
        if (reacquired) {
            reacquiring_ = false;
            onset_armed_ = false;
            onset_samples_ = 0;
            pitch_lock_frames_ = static_cast<int>(sample_rate_ * 0.45f);
            pending_count_ = 0;
            pending_hz_ = 0.0f;
        }
    }
}

void YinTracker::analyze() noexcept {
    if (reacquiring_) {
        if (onset_reduced_samples_ >= kReacquireWindow + max_lag_ + 1) {
            const auto candidate = analyze_buffer(residual_ring_, kReacquireWindow);
            if (candidate.valid && cents_apart(candidate.hz, output_.hz) >= 150.0f)
                accept_pitch(candidate, true);
            else { reacquire_pending_count_ = 0; reacquire_pending_hz_ = 0.0f; }
        }
        return; // hold old pitch, or return newly accepted reacquisition
    }
    const auto ordinary = analyze_buffer(ring_, kDiffWindow);
    if (pitch_lock_frames_ > 0 && (!ordinary.valid || cents_apart(ordinary.hz, output_.hz) > 80.0f))
        return;
    if (ordinary.valid) accept_pitch(ordinary, false);
    else output_.valid = false;
}

PitchEstimate YinTracker::process_sample(float input) noexcept {
    if (pitch_lock_frames_ > 0) --pitch_lock_frames_;
    if (!std::isfinite(input)) input = 0.0f;
    dc_ += highpass_alpha_ * (input - dc_);
    const float hp = input - dc_;
    const float level = std::abs(hp);
    const float env_alpha = level > envelope_ ? env_attack_alpha_ : env_release_alpha_;
    envelope_ += env_alpha * (level - envelope_);
    const bool previous_gate = output_.gate;
    if (envelope_ >= kGateThreshold) {
        gate_quiet_samples_ = 0;
        output_.gate = true;
    } else if (output_.gate && ++gate_quiet_samples_ >= gate_release_samples_) {
        output_.gate = false;
        output_.hz = 0.0f;
        output_.confidence = 0.0f;
        output_.valid = false;
        pending_hz_ = reacquire_pending_hz_ = 0.0f;
        pending_count_ = reacquire_pending_count_ = 0;
        reacquiring_ = false;
        onset_armed_ = false;
        pitch_lock_frames_ = 0;
    }
    if (!previous_gate && output_.gate) {
        ring_count_ = ring_head_ = since_analysis_ = 0;
        output_.hz = output_.confidence = 0.0f;
        output_.valid = false;
        pending_count_ = 0;
        pending_hz_ = 0.0f;
        onset_armed_ = reacquiring_ = false;
        pitch_lock_frames_ = 0;
    }

    // The novelty/onset detector runs after low-pass filtering and
    // decimation in push_downsampled(). It compares the signal with a
    // one-period-delayed copy of the previously tracked note.
    if (onset_samples_ < reacquire_timeout_samples_ + onset_cooldown_samples_)
        ++onset_samples_;
    if (reacquiring_ && onset_samples_ > reacquire_timeout_samples_)
        reacquiring_ = false;

    lp_a_ += lowpass_alpha_ * (hp - lp_a_);
    lp_b_ += lowpass_alpha_ * (lp_a_ - lp_b_);
    lp_c_ += lowpass_alpha_ * (lp_b_ - lp_c_);
    lp_d_ += lowpass_alpha_ * (lp_c_ - lp_d_);
    if (++decimate_counter_ >= decimate_factor_) {
        decimate_counter_ = 0;
        if (output_.gate) push_downsampled(lp_d_);
    }
    return output_;
}

PitchEstimate YinTracker::process_block(const float* mono, std::size_t frames) noexcept {
    if (mono == nullptr) return output_;
    for (std::size_t i = 0; i < frames; ++i) process_sample(mono[i]);
    return output_;
}

} // namespace wyldtone
