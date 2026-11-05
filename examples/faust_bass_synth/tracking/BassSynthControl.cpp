// SPDX-License-Identifier: Apache-2.0
// Native C++ sidecar for the Faust Bass Synth. A single YIN tracker feeds
// two internal Faust signal channels: pitch in Hz and the note gate.
#include "YinTracker.h"

#include <cmath>
#include <cstring>

namespace {
wyldtone::YinTracker tracker{48000.0f};
int trackingSource = 0; // 0: stereo sum, 1: left, 2: right
} // namespace

extern "C" void wyldtone_faust_extra_prepare(int sampleRate) {
    tracker.prepare(static_cast<float>(sampleRate));
    trackingSource = 0;
}
extern "C" void wyldtone_faust_extra_reset() {
    tracker.reset();
}
extern "C" void wyldtone_faust_extra_parameter(const char* id, float value) {
    if (!id || !std::isfinite(value)) return;
    if (std::strncmp(id, "tracking_source", 15) == 0) {
        trackingSource = value < 0.5f ? 0 : (value < 1.5f ? 1 : 2);
    } else if (std::strncmp(id, "tracking_response", 17) == 0) {
        tracker.set_tracking_response_ms(value);
    }
}
extern "C" void wyldtone_faust_extra_process(const float* const* mainInputs,
    int mainChannels, int frames, float* const* extraOutputs, int extraChannels) {
    if (!extraOutputs || extraChannels != 2 || !extraOutputs[0] || !extraOutputs[1]
        || !mainInputs || mainChannels < 1 || !mainInputs[0] || frames < 0) return;
    const float* left = mainInputs[0];
    const float* right = mainChannels > 1 && mainInputs[1] ? mainInputs[1] : left;
    for (int n = 0; n < frames; ++n) {
        const float input = trackingSource == 1 ? left[n]
            : (trackingSource == 2 ? right[n] : 0.5f * (left[n] + right[n]));
        const auto pitch = tracker.process_sample(input);
        // The detector may temporarily report an uncertain frame. Continue
        // the last accepted note until the input gate closes; after a pause,
        // the tracker explicitly clears its previous pitch on the new attack.
        const bool noteReady = pitch.gate && pitch.hz > 0.0f && std::isfinite(pitch.hz);
        extraOutputs[0][n] = noteReady ? pitch.hz : 0.0f;
        extraOutputs[1][n] = noteReady ? 1.0f : 0.0f;
    }
}
