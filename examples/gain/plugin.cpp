#include "wasm_plugin.h"
#include <stdint.h>
#include <math.h>
#include <string.h>

// Minimal, self-contained legacy ABI example. No dependency on the private engine.
namespace {
float audio_buffer[16384] = {};
float gain_db = 0.0f;
int32_t channel_count = 2;
int32_t copy_string(const char* text, int32_t dst, int32_t capacity) {
    if (dst <= 0 || capacity <= 0) return 0;
    const int32_t length = static_cast<int32_t>(strlen(text));
    const int32_t count = length < capacity - 1 ? length : capacity - 1;
    memcpy(reinterpret_cast<void*>(static_cast<uintptr_t>(dst)), text, static_cast<size_t>(count));
    reinterpret_cast<char*>(static_cast<uintptr_t>(dst))[count] = 0;
    return count;
}
}
extern "C" {
void plugin_init(int32_t, int32_t channels, int32_t) {
    channel_count = channels == 1 ? 1 : 2;
    gain_db = 0.0f;
}
// Declare one stereo input bus and one stereo output bus.
// Without these optional ABI exports, the host assumes legacy mono.
WASM_PLUGIN_EXPORT int32_t plugin_input_bus_count(void) { return 1; }
WASM_PLUGIN_EXPORT int32_t plugin_output_bus_count(void) { return 1; }
WASM_PLUGIN_EXPORT int32_t plugin_input_bus_channels(int32_t bus_index) {
    return bus_index == 0 ? 2 : 0;
}
WASM_PLUGIN_EXPORT int32_t plugin_output_bus_channels(int32_t bus_index) {
    return bus_index == 0 ? 2 : 0;
}
int32_t plugin_name(int32_t ptr, int32_t size) { return copy_string("SDK Gain Example", ptr, size); }
int32_t plugin_buffer_ptr() { return static_cast<int32_t>(reinterpret_cast<uintptr_t>(audio_buffer)); }
void plugin_process(int32_t ptr, int32_t samples) {
    if (ptr != plugin_buffer_ptr() || samples < 0 || samples > 16384) return;
    const float gain = powf(10.0f, gain_db / 20.0f);
    for (int32_t i = 0; i < samples; ++i) audio_buffer[i] *= gain;
}
int32_t plugin_parameter_count() { return 1; }
int32_t plugin_parameter_id(int32_t i, int32_t ptr, int32_t len) { return i == 0 ? copy_string("gain", ptr, len) : 0; }
int32_t plugin_parameter_name(int32_t i, int32_t ptr, int32_t len) { return i == 0 ? copy_string("Gain", ptr, len) : 0; }
int32_t plugin_parameter_label(int32_t i, int32_t ptr, int32_t len) { return i == 0 ? copy_string("dB", ptr, len) : 0; }
float plugin_parameter_min(int32_t) { return -24.0f; }
float plugin_parameter_max(int32_t) { return 24.0f; }
float plugin_parameter_default(int32_t) { return 0.0f; }
float plugin_parameter_step(int32_t) { return 0.1f; }
int32_t plugin_parameter_flags(int32_t) { return 1; }
float plugin_parameter_get(int32_t) { return (gain_db + 24.0f) / 48.0f; }
void plugin_parameter_set(int32_t i, float normalized) {
    if (i != 0) return;
    if (normalized < 0) normalized = 0;
    if (normalized > 1) normalized = 1;
    gain_db = -24.0f + normalized * 48.0f;
}
int32_t plugin_pending_events() { return 0; }
}
