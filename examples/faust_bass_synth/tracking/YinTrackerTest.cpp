// SPDX-License-Identifier: Apache-2.0
#include "YinTracker.h"

#include <cmath>
#include <iostream>
#include <stdexcept>
#include <string>

namespace {
constexpr double kPi = 3.1415926535897932384626433832795;
void require(bool ok, const std::string& why) {
    if (!ok) throw std::runtime_error(why);
}
void one_test(float sample_rate) {
    wyldtone::YinTracker tracker(sample_rate);
    const float notes[] = {41.2034f, 55.0f, 73.4162f, 97.9989f};
    const int silence_frames = static_cast<int>(sample_rate * 0.25f);
    const int tone_frames = static_cast<int>(sample_rate * 0.95f);

    for (float note : notes) {
        for (int i = 0; i < silence_frames; ++i) {
            const auto out = tracker.process_sample(0.0f);
            if (i > silence_frames - static_cast<int>(sample_rate * 0.08f))
                require(!out.gate && !out.valid && out.hz == 0.0f, "stale note after silence");
        }
        int total = 0, correct = 0;
        for (int i = 0; i < tone_frames; ++i) {
            const double t = static_cast<double>(i) / sample_rate;
            // A bass-like signal with a weak fundamental and strong harmonics.
            const float sample = static_cast<float>(
                0.018 * std::sin(2.0 * kPi * note * t) +
                0.04 * std::sin(2.0 * kPi * 2 * note * t + 0.3) +
                0.07 * std::sin(2.0 * kPi * 3 * note * t + 0.6) +
                0.035 * std::sin(2.0 * kPi * 4 * note * t));
            const auto out = tracker.process_sample(sample);
            if (i > static_cast<int>(sample_rate * 0.25f) && i % 128 == 0) {
                ++total;
                if (out.valid && out.gate && out.confidence >= 0.8f && out.hz > 0.0f &&
                    std::abs(1200.0 * std::log2(out.hz / note)) < 60.0) ++correct;
            }
        }
        require(total > 0 && correct > 0.82 * total,
                "wrong or missing note for " + std::to_string(note) + " at " +
                std::to_string(sample_rate) + " Hz: " + std::to_string(correct) + "/" +
                std::to_string(total));
    }
}
} // namespace

int main() {
    try {
        for (float sample_rate : {44100.0f, 48000.0f, 96000.0f}) {
            one_test(sample_rate);
            std::cout << "PASS synthetic E1 A1 D2 G2 @ " << sample_rate << " Hz\n";
        }
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "FAIL " << error.what() << '\n';
        return 1;
    }
}
