// SPDX-License-Identifier: Apache-2.0
#pragma once

#include <array>
#include <cstddef>

namespace wyldtone {

struct PitchEstimate {
    float hz = 0.0f;
    float confidence = 0.0f;
    bool gate = false;
    bool valid = false;
};

// Monophonic tracker with optional onset-triggered reacquisition when a
// previous string continues ringing. No allocation, locks, or threads.
class YinTracker {
public:
    explicit YinTracker(float sample_rate = 48000.0f);
    void prepare(float sample_rate) noexcept;
    void reset() noexcept;
    // Smaller values react more readily, larger values reject questionable
    // changes. Does not change the detector's intrinsic analysis-window delay.
    void set_tracking_response_ms(float milliseconds) noexcept;
    PitchEstimate process_sample(float input) noexcept;
    PitchEstimate process_block(const float* mono, std::size_t frames) noexcept;
    PitchEstimate current() const noexcept { return output_; }

private:
    struct Candidate { float hz = 0.0f; float confidence = 0.0f; bool valid = false; };
    static constexpr int kRingSize = 1024;
    static constexpr int kDiffWindow = 384;
    static constexpr int kReacquireWindow = 224;
    static constexpr int kMaxLag = 256;
    static constexpr float kMinPitchHz = 30.0f;
    static constexpr float kMaxPitchHz = 250.0f;
    static constexpr float kGateThreshold = 0.0025f;

    void push_downsampled(float x) noexcept;
    void analyze() noexcept;
    Candidate analyze_buffer(const std::array<float, kRingSize>& ring, int window) noexcept;
    float sample_from_latest(const std::array<float, kRingSize>& ring, int delay) const noexcept;
    void accept_pitch(Candidate candidate, bool reacquired) noexcept;
    void start_reacquisition() noexcept;

    std::array<float, kRingSize> ring_{};
    std::array<float, kRingSize> residual_ring_{};
    std::array<float, kMaxLag + 2> cmnd_{};
    int ring_head_ = 0;
    int ring_count_ = 0;
    int decimate_factor_ = 8;
    int decimate_counter_ = 0;
    int since_analysis_ = 0;
    int analysis_hop_ = 16;
    int gate_quiet_samples_ = 0;
    int gate_release_samples_ = 0;
    int max_lag_ = 200;
    int min_lag_ = 24;
    int onset_samples_ = 0;
    int onset_reduced_samples_ = 0;
    int onset_cooldown_samples_ = 0;
    int reacquire_timeout_samples_ = 0;
    int reacquire_confirmations_ = 2;
    int pitch_lock_frames_ = 0;
    bool onset_armed_ = false;
    bool reacquiring_ = false;
    float comb_period_samples_ = 0.0f;
    float sample_rate_ = 48000.0f;
    float reduced_sample_rate_ = 6000.0f;
    float highpass_alpha_ = 0.0f;
    float lowpass_alpha_ = 0.0f;
    float env_attack_alpha_ = 0.0f;
    float env_release_alpha_ = 0.0f;
    float novelty_fast_alpha_ = 0.0f;
    float novelty_slow_alpha_ = 0.0f;
    float novelty_fast_ = 0.0f;
    float novelty_slow_ = 0.0f;
    float signal_fast_ = 0.0f;
    float dc_ = 0.0f;
    float lp_a_ = 0.0f;
    float lp_b_ = 0.0f;
    float lp_c_ = 0.0f;
    float lp_d_ = 0.0f;
    float envelope_ = 0.0f;
    float pending_hz_ = 0.0f;
    int pending_count_ = 0;
    float reacquire_pending_hz_ = 0.0f;
    int reacquire_pending_count_ = 0;
    PitchEstimate output_{};
};

} // namespace wyldtone
