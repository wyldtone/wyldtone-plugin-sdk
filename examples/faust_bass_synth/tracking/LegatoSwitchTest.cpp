// SPDX-License-Identifier: Apache-2.0
// Synthetic string-overlap regression: the old note continues after a new
// bass string is plucked. The tracker must reacquire the newly played pitch.
#include "YinTracker.h"

#include <cmath>
#include <cstdio>
#include <algorithm>
#include <utility>
#include <initializer_list>
#include <stdexcept>

namespace {
constexpr double pi = 3.14159265358979323846;
float bass(float hz, double seconds) {
    return static_cast<float>(
        0.025 * std::sin(2.0 * pi * hz * seconds) +
        0.040 * std::sin(4.0 * pi * hz * seconds + 0.14) +
        0.065 * std::sin(6.0 * pi * hz * seconds + 0.35) +
        0.018 * std::sin(8.0 * pi * hz * seconds));
}
bool matches(float measured, float wanted) {
    return std::isfinite(measured) && measured > 0.0f &&
        std::abs(1200.0f * std::log2(measured / wanted)) < 65.0f;
}
void overlapping_transition(float rate, float from, float to,
                            float new_gain, float old_decay) {
    wyldtone::YinTracker tracker(rate);
    const int before = static_cast<int>(rate * 0.48f);
    const int after = static_cast<int>(rate * 0.55f);
    int correct = 0;
    int checked = 0;
    float first_ms = -1.0f;
    bool initial_correct = false;
    for (int n = 0; n < before + after; ++n) {
        float signal = 0.0f;
        if (n < before) signal = bass(from, static_cast<double>(n) / rate);
        else {
            const double dt = static_cast<double>(n - before) / rate;
            const float leftover = std::exp(-static_cast<float>(dt / old_decay));
            const float attack = std::min(1.0f, static_cast<float>(dt / 0.014));
            signal = leftover * bass(from, static_cast<double>(n) / rate)
                   + new_gain * attack * bass(to, dt);
        }
        const auto result = tracker.process_sample(signal);
        if (n == before - 1) initial_correct = result.valid && matches(result.hz, from);
        if (n > before && first_ms < 0.0f && result.valid && matches(result.hz, to))
            first_ms = 1000.0f * static_cast<float>(n - before) / rate;
        if (n > before + static_cast<int>(rate * 0.17f) && n % 128 == 0) {
            ++checked;
            if (result.valid && result.gate && matches(result.hz, to)) ++correct;
        }
    }
    std::printf("%.0f Hz / %.1f -> %.1f Hz / new gain %.2f: first %.1f ms, stable %d/%d\n",
                rate, from, to, new_gain, first_ms, correct, checked);
    if (!initial_correct || checked < 10 || first_ms < 0.0f || first_ms > 150.0f ||
        correct < static_cast<int>(checked * 0.90f))
        throw std::runtime_error("pitch reacquisition failed with ringing string");
}
void steady_note(float rate, float hz) {
    wyldtone::YinTracker tracker(rate);
    int checked = 0, correct = 0;
    for (int n = 0; n < static_cast<int>(rate * 1.4f); ++n) {
        const double t = static_cast<double>(n) / rate;
        const float amplitude = 1.0f + 0.08f * std::sin(2.0 * pi * 3.0 * t);
        const auto result = tracker.process_sample(amplitude * bass(hz, t));
        if (t > 0.3 && n % 128 == 0) {
            ++checked;
            if (result.valid && matches(result.hz, hz)) ++correct;
        }
    }
    if (correct < static_cast<int>(checked * 0.95f))
        throw std::runtime_error("steady note tracking regressed");
}
}
int main() {
    try {
        for (float rate : {44100.0f, 48000.0f, 96000.0f}) {
            for (const auto& pair : {std::pair<float,float>{41.2034f,55.0f},
                                     {55.0f,73.4162f}, {73.4162f,97.9989f},
                                     {97.9989f,41.2034f}}) {
                overlapping_transition(rate, pair.first, pair.second, 1.0f, 0.23f);
                overlapping_transition(rate, pair.first, pair.second, 0.65f, 0.42f);
            }
            steady_note(rate, 41.2034f);
            steady_note(rate, 97.9989f);
        }
        std::puts("PASS held notes and transitions with overlapping bass strings");
        return 0;
    } catch (const std::exception& ex) {
        std::fprintf(stderr, "FAIL: %s\n", ex.what());
        return 1;
    }
}
