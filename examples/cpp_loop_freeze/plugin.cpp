// SPDX-License-Identifier: Apache-2.0
// Wyldtone Loop Freeze v0.1: click-reduced, stereo live-buffer sustain.
// Stereo interleaved legacy ABI. All state is preallocated; no malloc/new in process.
#include "wasm_plugin.h"
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>

namespace {
constexpr int kChannels = 2;
constexpr int kBufferFloats = 16384;
constexpr int kRingFrames = 524288;  // 4.0 MiB for two float channels, power of two.
constexpr int kMask = kRingFrames - 1;
constexpr float kPi = 3.14159265358979323846f;
float sharedBuffer[kBufferFloats]{};
float history[kChannels][kRingFrames]{};

struct Parameter {
    const char* id;
    const char* name;
    const char* unit;
    float minValue, maxValue, defaultValue, step;
    bool boolean;
};
constexpr Parameter kParams[] = {
    {"freeze",         "Freeze",          "",   0,     1,    0,     1,  true},
    {"length_ms",      "Capture Length",  "ms", 80,    900,  320,   10, false},
    {"crossfade_ms",   "Seam Crossfade", "ms", 5,     100,  35,    1, false},
    {"attack_ms",      "Freeze Attack",  "ms", 5,     300,  65,    1, false},
    {"release_ms",     "Freeze Release", "ms", 10,   1000,  220,  10, false},
    {"dry_level",      "Dry Level",       "",  0,     1,    1.0f, 0.01f, false},
    {"freeze_level",   "Freeze Level",    "",  0,     1,    0.85f, 0.01f, false},
    {"tone_hz",        "Freeze Tone",    "Hz", 500, 15000, 11000,  100, false},
};
constexpr int kParameterCount = sizeof(kParams) / sizeof(kParams[0]);
float values[kParameterCount]{};
int sampleRate = 48000;
int writeIndex = 0;
int validFrames = 0;
int startIndex = 0;
int capturedFrames = 0;
int crossfadeFrames = 0;
int loopIndex = 0;
float holdEnvelope = 0.0f;
float smoothWet[2]{};
bool active = false;
bool previousFreeze = false;

int textCopy(const char* text, int ptr, int capacity) {
    if (ptr <= 0 || capacity <= 0 || !text) return 0;
    const int n = std::min(static_cast<int>(std::strlen(text)), capacity - 1);
    auto* dest = reinterpret_cast<char*>(static_cast<uintptr_t>(ptr));
    std::memcpy(dest, text, static_cast<size_t>(n));
    dest[n] = '\0';
    return n;
}

void resetState() {
    // Called only on plugin_init/reset, never on audio callback.
    std::memset(history, 0, sizeof(history));
    writeIndex = validFrames = startIndex = capturedFrames = crossfadeFrames = loopIndex = 0;
    holdEnvelope = 0.0f;
    smoothWet[0] = smoothWet[1] = 0.0f;
    active = previousFreeze = false;
}

void capture() {
    const int wanted = std::clamp(
        static_cast<int>(std::lround(values[1] * sampleRate * 0.001f)), 1, kRingFrames - 1);
    capturedFrames = std::min(wanted, validFrames);
    if (capturedFrames < 32) { active = false; return; }
    crossfadeFrames = std::clamp(
        static_cast<int>(std::lround(values[2] * sampleRate * 0.001f)),
        1, std::max(1, capturedFrames / 4));
    startIndex = (writeIndex - capturedFrames) & kMask;
    loopIndex = crossfadeFrames;
    active = true;
    smoothWet[0] = smoothWet[1] = 0.0f;
}

float heldSample(int ch) {
    if (!active) return 0.0f;
    const int sampleAt = (startIndex + loopIndex) & kMask;
    float sample = history[ch][sampleAt];
    if (loopIndex >= capturedFrames - crossfadeFrames) {
        const float u = float(loopIndex - (capturedFrames - crossfadeFrames))
                      / float(crossfadeFrames);
        // Constant-power overlap is often louder if the two regions correlate.
        // Linear overlap is safer for a raw freeze: no extra gain at the seam.
        const float head = history[ch][(startIndex + loopIndex -
                                       (capturedFrames - crossfadeFrames)) & kMask];
        sample += u * (head - sample);
    }
    return sample;
}

void advanceLoop() {
    if (!active) return;
    ++loopIndex;
    if (loopIndex >= capturedFrames) loopIndex = crossfadeFrames;
}

void processInterleaved(float* audio, int frames) {
    if (!audio || frames <= 0) return;
    const bool wantsFreeze = values[0] >= 0.5f;
    const float sr = float(sampleRate);
    const float rise = 1.0f / std::max(1.0f, values[3] * 0.001f * sr);
    const float fall = 1.0f / std::max(1.0f, values[4] * 0.001f * sr);
    const float cutoff = std::min(values[7], 0.45f * sr);
    const float toneCoef = 1.0f - std::exp(-2.0f * kPi * cutoff / sr);
    const float dry = values[5];
    const float frozen = values[6];

    for (int i = 0; i < frames; ++i) {
        float in[2] = {audio[2 * i], audio[2 * i + 1]};
        for (float& sample : in) if (!std::isfinite(sample)) sample = 0.0f;

        if (wantsFreeze && !previousFreeze && !active) capture();
        // Once captured, the ring is held until the release envelope reaches zero.
        // The history is not overwritten while it is still audible.
        if (wantsFreeze && active) holdEnvelope = std::min(1.0f, holdEnvelope + rise);
        else holdEnvelope = std::max(0.0f, holdEnvelope - fall);

        for (int ch = 0; ch < kChannels; ++ch) {
            float wet = heldSample(ch);
            smoothWet[ch] += toneCoef * (wet - smoothWet[ch]);
            audio[2 * i + ch] = in[ch] * dry + smoothWet[ch] * frozen * holdEnvelope;
        }
        advanceLoop();

        if (active && !wantsFreeze && holdEnvelope <= 0.0f) {
            active = false;
            smoothWet[0] = smoothWet[1] = 0.0f;
        }
        if (!active) {
            history[0][writeIndex] = in[0];
            history[1][writeIndex] = in[1];
            writeIndex = (writeIndex + 1) & kMask;
            validFrames = std::min(validFrames + 1, kRingFrames);
        }
        previousFreeze = wantsFreeze;
    }
}
}  // namespace

extern "C" {
void plugin_init(int rate, int, int) {
    sampleRate = (rate >= 8000 && rate <= 384000) ? rate : 48000;
    for (int i = 0; i < kParameterCount; ++i) values[i] = kParams[i].defaultValue;
    resetState();
}
WASM_PLUGIN_EXPORT void plugin_reset() { resetState(); }
int plugin_name(int ptr, int capacity) { return textCopy("Wyldtone Loop Freeze", ptr, capacity); }
int plugin_buffer_ptr() { return static_cast<int>(reinterpret_cast<uintptr_t>(sharedBuffer)); }
void plugin_process(int ptr, int numberOfFloats) {
    if (ptr != plugin_buffer_ptr() || numberOfFloats < 0 ||
        numberOfFloats > kBufferFloats || (numberOfFloats % 2)) return;
    processInterleaved(sharedBuffer, numberOfFloats / 2);
}
int plugin_parameter_count() { return kParameterCount; }
int plugin_parameter_id(int index, int ptr, int size) {
    return (index >= 0 && index < kParameterCount) ? textCopy(kParams[index].id, ptr, size) : 0;
}
int plugin_parameter_name(int index, int ptr, int size) {
    return (index >= 0 && index < kParameterCount) ? textCopy(kParams[index].name, ptr, size) : 0;
}
int plugin_parameter_label(int index, int ptr, int size) {
    return (index >= 0 && index < kParameterCount) ? textCopy(kParams[index].unit, ptr, size) : 0;
}
float plugin_parameter_min(int index) { return (index >= 0 && index < kParameterCount) ? kParams[index].minValue : 0.0f; }
float plugin_parameter_max(int index) { return (index >= 0 && index < kParameterCount) ? kParams[index].maxValue : 1.0f; }
float plugin_parameter_default(int index) { return (index >= 0 && index < kParameterCount) ? kParams[index].defaultValue : 0.0f; }
float plugin_parameter_step(int index) { return (index >= 0 && index < kParameterCount) ? kParams[index].step : 0.0f; }
int plugin_parameter_flags(int index) { return (index >= 0 && index < kParameterCount) ? (1 | (kParams[index].boolean ? 4 : 0)) : 0; }
float plugin_parameter_get(int index) {
    if (index < 0 || index >= kParameterCount) return 0.0f;
    const auto& p = kParams[index];
    return (values[index] - p.minValue) / (p.maxValue - p.minValue);
}
void plugin_parameter_set(int index, float normalized) {
    if (index < 0 || index >= kParameterCount || !std::isfinite(normalized)) return;
    const auto& p = kParams[index];
    normalized = std::clamp(normalized, 0.0f, 1.0f);
    float value = p.minValue + normalized * (p.maxValue - p.minValue);
    if (p.boolean) value = value >= 0.5f ? 1.0f : 0.0f;
    else if (p.step > 0.0f) value = p.minValue + std::round((value - p.minValue) / p.step) * p.step;
    values[index] = std::clamp(value, p.minValue, p.maxValue);
}
int plugin_pending_events() { return 0; }
}
