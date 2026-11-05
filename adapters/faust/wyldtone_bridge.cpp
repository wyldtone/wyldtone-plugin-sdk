// Wyldtone/Faust ABI bridge. The generated C++ code is included as a single
// translation unit; no original Faust DSP sources are modified.
#include <emscripten/emscripten.h>
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <cctype>
#include <string>
#include <unordered_map>
#include <vector>

#include "faust_generated.cpp"

#ifndef WYLDTONE_FAUST_PLUGIN_NAME
#define WYLDTONE_FAUST_PLUGIN_NAME "Faust Plugin"
#endif

namespace {
#ifndef WYLDTONE_FAUST_EXTRA_INPUTS
#define WYLDTONE_FAUST_EXTRA_INPUTS 0
#endif
#ifndef WYLDTONE_FAUST_MAIN_INPUTS
#define WYLDTONE_FAUST_MAIN_INPUTS 0
#endif
#if WYLDTONE_FAUST_EXTRA_INPUTS > 0
// Implemented by plugin-local native control sources; fills additional Faust
// signal inputs without changing the host-visible input/output buses.
extern "C" void wyldtone_faust_extra_prepare(int sampleRate);
extern "C" void wyldtone_faust_extra_reset();
extern "C" void wyldtone_faust_extra_process(const float* const* mainInputs,
    int mainChannels, int frames, float* const* extraOutputs, int extraChannels);
extern "C" void wyldtone_faust_extra_parameter(const char* id, float value);
#endif

constexpr int kMaxChannels = 8;
constexpr int kMaxFrames = 2048;
constexpr int kBufferFloats = 16384;
float abiBuffer[kBufferFloats]{};
float planeInputs[kMaxChannels][kMaxFrames]{};
float planeOutputs[kMaxChannels][kMaxFrames]{};
FAUSTFLOAT* inputPtrs[kMaxChannels]{};
FAUSTFLOAT* outputPtrs[kMaxChannels]{};
WyldFaustDSP processor;

struct Control {
    std::string id, name, unit;
    FAUSTFLOAT* zone{};
    float initial{}, min{}, max{}, step{};
    bool boolean{};
};

std::string toId(const std::string& s) {
    std::string out;
    bool separator = false;
    for (unsigned char c : s) {
        if (std::isalnum(c) && c < 128) {
            if (separator && !out.empty()) out += '_';
            out += char(std::tolower(c));
            separator = false;
        } else separator = true;
    }
    if (out.empty()) out = "param";
    return out;
}

struct HostUI final : UI {
    std::vector<Control> controls;
    std::vector<std::string> groups;
    std::unordered_map<const FAUSTFLOAT*, std::string> units;

    void openTabBox(const char* s) override { groups.emplace_back(s ? s : ""); }
    void openHorizontalBox(const char* s) override { groups.emplace_back(s ? s : ""); }
    void openVerticalBox(const char* s) override { groups.emplace_back(s ? s : ""); }
    void closeBox() override { if (!groups.empty()) groups.pop_back(); }

    void push(const char* label, FAUSTFLOAT* zone, float initial,
              float minimum, float maximum, float step, bool boolean) {
        if (!zone || controls.size() >= 256) return;
        std::string path;
        for (auto& group : groups) { if (!group.empty()) { path += group; path += '/'; } }
        path += label ? label : "Parameter";
        std::string id = toId(path);
        for (const auto& p : controls) { if (p.id == id) { id += "_" + std::to_string(controls.size()); break; } }
        Control p;
        p.id = std::move(id);
        p.name = label ? label : "Parameter";
        p.zone = zone;
        p.initial = initial;
        p.min = minimum;
        p.max = maximum;
        p.step = step;
        p.boolean = boolean;
        if (auto it = units.find(zone); it != units.end()) p.unit = it->second;
        controls.push_back(std::move(p));
    }
    void addButton(const char* s, FAUSTFLOAT* z) override { push(s, z, 0, 0, 1, 1, true); }
    void addCheckButton(const char* s, FAUSTFLOAT* z) override { push(s, z, 0, 0, 1, 1, true); }
    void addVerticalSlider(const char* s, FAUSTFLOAT* z, FAUSTFLOAT d, FAUSTFLOAT a, FAUSTFLOAT b, FAUSTFLOAT st) override {
        push(s, z, d, a, b, st, false);
    }
    void addHorizontalSlider(const char* s, FAUSTFLOAT* z, FAUSTFLOAT d, FAUSTFLOAT a, FAUSTFLOAT b, FAUSTFLOAT st) override {
        push(s, z, d, a, b, st, false);
    }
    void addNumEntry(const char* s, FAUSTFLOAT* z, FAUSTFLOAT d, FAUSTFLOAT a, FAUSTFLOAT b, FAUSTFLOAT st) override {
        push(s, z, d, a, b, st, false);
    }
    void addHorizontalBargraph(const char*, FAUSTFLOAT*, FAUSTFLOAT, FAUSTFLOAT) override { }
    void addVerticalBargraph(const char*, FAUSTFLOAT*, FAUSTFLOAT, FAUSTFLOAT) override { }
    // Soundfile widgets require a host-side filesystem integration; not supported in v0.1.
    void addSoundfile(const char*, const char*, Soundfile**) override { }
    void declare(FAUSTFLOAT* zone, const char* key, const char* value) override {
        if (zone && key && value && std::strcmp(key, "unit") == 0) {
            units[zone] = value;
            for (auto& c : controls) if (c.zone == zone) c.unit = value;
        }
    }
};

HostUI ui;
bool ready = false;
int sampleRate = 48000;

int mainInputs() {
    const int total = processor.getNumInputs();
#if WYLDTONE_FAUST_EXTRA_INPUTS > 0
    // A mismatched Faust DSP layout must fail the SDK bus-layout inspection
    // instead of silently generating a stereo plugin that cannot process audio.
    if (total != WYLDTONE_FAUST_MAIN_INPUTS + WYLDTONE_FAUST_EXTRA_INPUTS) return 0;
    return WYLDTONE_FAUST_MAIN_INPUTS;
#else
    return total;
#endif
}

void ensureReady() {
    if (ready) return;
    processor.init(sampleRate);
    processor.buildUserInterface(&ui);
#if WYLDTONE_FAUST_EXTRA_INPUTS > 0
    wyldtone_faust_extra_prepare(sampleRate);
    for (const auto& c : ui.controls)
        wyldtone_faust_extra_parameter(c.id.c_str(), float(*c.zone));
#endif
    ready = true;
}

int copyText(const std::string& s, int ptr, int maxBytes) {
    if (ptr <= 0 || maxBytes <= 0) return 0;
    const size_t size = std::min(s.size(), size_t(maxBytes - 1));
    auto* dest = reinterpret_cast<char*>(static_cast<uintptr_t>(ptr));
    std::memcpy(dest, s.data(), size);
    dest[size] = 0;
    return int(size);
}

Control* findControl(int i) {
    ensureReady();
    if (i < 0 || size_t(i) >= ui.controls.size()) return nullptr;
    return &ui.controls[size_t(i)];
}

void processBlock(const float* src, float* dst, int frames) {
    ensureReady();
    const int dspInputs = processor.getNumInputs();
    const int ins = mainInputs();
    const int outs = processor.getNumOutputs();
    if (frames < 0 || frames > kMaxFrames || ins < 0 || outs < 0 ||
        ins > kMaxChannels || outs > kMaxChannels ||
        dspInputs != ins + WYLDTONE_FAUST_EXTRA_INPUTS || dspInputs > kMaxChannels) return;
    for (int ch = 0; ch < ins; ++ch) {
        inputPtrs[ch] = planeInputs[ch];
        for (int f = 0; f < frames; ++f) planeInputs[ch][f] = src[f * ins + ch];
    }
#if WYLDTONE_FAUST_EXTRA_INPUTS > 0
    const float* mainPtrs[kMaxChannels]{};
    float* auxPtrs[kMaxChannels]{};
    for (int ch = 0; ch < ins; ++ch) mainPtrs[ch] = planeInputs[ch];
    for (int ch = 0; ch < WYLDTONE_FAUST_EXTRA_INPUTS; ++ch)
        auxPtrs[ch] = planeInputs[ins + ch];
    wyldtone_faust_extra_process(mainPtrs, ins, frames, auxPtrs, WYLDTONE_FAUST_EXTRA_INPUTS);
#endif
    for (int ch = ins; ch < dspInputs; ++ch) inputPtrs[ch] = planeInputs[ch];
    for (int ch = 0; ch < outs; ++ch) outputPtrs[ch] = planeOutputs[ch];
    processor.compute(frames, inputPtrs, outputPtrs);
    for (int f = 0; f < frames; ++f)
        for (int ch = 0; ch < outs; ++ch) dst[f * outs + ch] = planeOutputs[ch][f];
}
} // namespace

extern "C" {
// Old and new host ABI exports. All are listed explicitly by the SDK builder.
EMSCRIPTEN_KEEPALIVE void plugin_create() { ensureReady(); }
EMSCRIPTEN_KEEPALIVE void plugin_init(int rate, int, int) {
    ensureReady();
    sampleRate = (rate >= 8000 && rate <= 384000) ? rate : 48000;
    // Reinitialise DSP state without invalidating parameter pointers.
    processor.init(sampleRate);
#if WYLDTONE_FAUST_EXTRA_INPUTS > 0
    wyldtone_faust_extra_prepare(sampleRate);
    for (const auto& c : ui.controls)
        wyldtone_faust_extra_parameter(c.id.c_str(), float(*c.zone));
#endif
}
EMSCRIPTEN_KEEPALIVE void plugin_reset() {
    ensureReady(); processor.instanceClear();
#if WYLDTONE_FAUST_EXTRA_INPUTS > 0
    wyldtone_faust_extra_reset();
#endif
}
EMSCRIPTEN_KEEPALIVE int plugin_name(int ptr, int len) { return copyText(WYLDTONE_FAUST_PLUGIN_NAME, ptr, len); }
EMSCRIPTEN_KEEPALIVE int plugin_buffer_ptr() { return int(reinterpret_cast<uintptr_t>(abiBuffer)); }
EMSCRIPTEN_KEEPALIVE int plugin_input_bus_count() { ensureReady(); return mainInputs() ? 1 : 0; }
EMSCRIPTEN_KEEPALIVE int plugin_output_bus_count() { ensureReady(); return processor.getNumOutputs() ? 1 : 0; }
EMSCRIPTEN_KEEPALIVE int plugin_input_bus_channels(int i) { ensureReady(); return i == 0 ? mainInputs() : 0; }
EMSCRIPTEN_KEEPALIVE int plugin_output_bus_channels(int i) { ensureReady(); return i == 0 ? processor.getNumOutputs() : 0; }
EMSCRIPTEN_KEEPALIVE void plugin_process_buses(int ptr, int frames) {
    ensureReady();
    const int ins = mainInputs(), outs = processor.getNumOutputs();
    if (ptr != plugin_buffer_ptr() || frames < 0 || frames > kMaxFrames ||
        (int64_t(ins) + outs) * frames > kBufferFloats) return;
    processBlock(abiBuffer, abiBuffer + size_t(ins) * frames, frames);
}
EMSCRIPTEN_KEEPALIVE void plugin_process(int ptr, int samples) {
    ensureReady();
    const int ins = mainInputs(), outs = processor.getNumOutputs();
    // Legacy interleaved buffer uses one in-place region, so it requires matched layouts.
    if (ptr != plugin_buffer_ptr() || ins <= 0 || outs != ins || samples < 0 ||
        samples > kBufferFloats || samples % ins != 0) return;
    const int frames = samples / ins;
    if (frames > kMaxFrames) return;
    // The output has to be separate from the input to avoid overwrite during deinterleave.
    processBlock(abiBuffer, abiBuffer, frames);
}
EMSCRIPTEN_KEEPALIVE int plugin_parameter_count() { ensureReady(); return int(ui.controls.size()); }
EMSCRIPTEN_KEEPALIVE int plugin_parameter_id(int i, int ptr, int len) { auto* p = findControl(i); return p ? copyText(p->id, ptr, len) : 0; }
EMSCRIPTEN_KEEPALIVE int plugin_parameter_name(int i, int ptr, int len) { auto* p = findControl(i); return p ? copyText(p->name, ptr, len) : 0; }
EMSCRIPTEN_KEEPALIVE int plugin_parameter_label(int i, int ptr, int len) { auto* p = findControl(i); return p ? copyText(p->unit, ptr, len) : 0; }
EMSCRIPTEN_KEEPALIVE float plugin_parameter_min(int i) { auto* p = findControl(i); return p ? p->min : 0; }
EMSCRIPTEN_KEEPALIVE float plugin_parameter_max(int i) { auto* p = findControl(i); return p ? p->max : 1; }
EMSCRIPTEN_KEEPALIVE float plugin_parameter_default(int i) { auto* p = findControl(i); return p ? p->initial : 0; }
EMSCRIPTEN_KEEPALIVE float plugin_parameter_step(int i) { auto* p = findControl(i); return p ? p->step : 0; }
EMSCRIPTEN_KEEPALIVE int plugin_parameter_flags(int i) { auto* p = findControl(i); return p ? (1 | (p->boolean ? 4 : 0)) : 0; }
EMSCRIPTEN_KEEPALIVE float plugin_parameter_get(int i) {
    auto* p = findControl(i); if (!p || !p->zone || p->max <= p->min) return 0;
    return std::clamp((float(*p->zone) - p->min) / (p->max - p->min), 0.0f, 1.0f);
}
EMSCRIPTEN_KEEPALIVE void plugin_parameter_set(int i, float v) {
    auto* p = findControl(i); if (!p || !p->zone) return;
    v = std::clamp(std::isfinite(v) ? v : 0.0f, 0.0f, 1.0f);
    float value = p->min + v * (p->max - p->min);
    if (p->boolean) value = value >= 0.5f ? 1.0f : 0.0f;
    else if (p->step > 0) value = p->min + std::round((value - p->min) / p->step) * p->step;
    value = std::clamp(value, p->min, p->max);
    *p->zone = FAUSTFLOAT(value);
#if WYLDTONE_FAUST_EXTRA_INPUTS > 0
    wyldtone_faust_extra_parameter(p->id.c_str(), value);
#endif
}
EMSCRIPTEN_KEEPALIVE int plugin_pending_events() { return 0; }
} // extern C
