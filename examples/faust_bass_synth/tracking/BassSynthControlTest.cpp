// SPDX-License-Identifier: Apache-2.0
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <initializer_list>

extern "C" void wyldtone_faust_extra_prepare(int);
extern "C" void wyldtone_faust_extra_reset();
extern "C" void wyldtone_faust_extra_parameter(const char*, float);
extern "C" void wyldtone_faust_extra_process(
    const float* const*, int, int, float* const*, int);

namespace {
constexpr double pi = 3.14159265358979323846;
void expect(bool c, const char* message) {
    if (!c) { std::fprintf(stderr, "FAIL: %s\n", message); std::exit(1); }
}
void render(double freq, int channels = 2) {
    constexpr int sampleRate = 48000, block = 128;
    wyldtone_faust_extra_reset();
    float left[block]{}, right[block]{}, pitch[block]{}, gate[block]{};
    const float* ins[] = {left, right};
    float* aux[] = {pitch, gate};
    unsigned correct = 0, checked = 0;
    for (int base = 0; base < sampleRate; base += block) {
        for (int i = 0; i < block; ++i) {
            double t = double(base + i) / sampleRate;
            left[i] = float(0.055 * std::sin(2 * pi * freq * t) +
                            0.08 * std::sin(2 * pi * 2 * freq * t));
            right[i] = channels == 2 ? left[i] : 0.0f;
        }
        wyldtone_faust_extra_process(ins, 2, block, aux, 2);
        if (base > sampleRate / 3) {
            const float found = pitch[block - 1];
            if (gate[block - 1] > 0.5f) {
                ++checked;
                if (std::abs(1200.0 * std::log2(found / freq)) < 60.0) ++correct;
            }
        }
    }
    expect(checked > 10 && correct >= 0.80 * checked, "tracking frequency incorrect");
    for (int frame = 0; frame < sampleRate / 4; frame += block) {
        for (int i = 0; i < block; ++i) left[i] = right[i] = 0.0f;
        wyldtone_faust_extra_process(ins, 2, block, aux, 2);
    }
    expect(gate[block - 1] == 0.0f && pitch[block - 1] == 0.0f,
           "stale note after silence");
}
}

int main() {
    wyldtone_faust_extra_prepare(48000);
    wyldtone_faust_extra_parameter("tracking_response", 25.0f);
    for (double f : {41.2034, 55.0, 73.4162, 97.9989}) render(f);
    wyldtone_faust_extra_parameter("tracking_response", 105.0f);
    render(55.0);
    wyldtone_faust_extra_parameter("tracking_source_0_mix_1_left_2_right", 1.0f);
    render(73.4162, 1);
    std::puts("PASS Faust/YIN auxiliary-channel integration E1 A1 D2 G2");
}
