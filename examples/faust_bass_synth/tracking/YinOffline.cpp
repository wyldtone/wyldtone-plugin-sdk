// SPDX-License-Identifier: Apache-2.0
// Offline test only: reads WAV, runs the exact streaming C++ tracker and
// compares four known regions to their expected bass-string pitches.
#include "YinTracker.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
struct Audio {
    unsigned sample_rate = 0;
    unsigned channels = 0;
    std::vector<float> interleaved;
};
struct Region {
    std::string name;
    double begin = 0.0;
    double end = 0.0;
    double expected_hz = 0.0;
};
struct Row {
    double seconds;
    float hz;
    float confidence;
    bool gate;
    bool valid;
};
std::uint16_t u16(const unsigned char* p) {
    return static_cast<std::uint16_t>(p[0] | (p[1] << 8));
}
std::uint32_t u32(const unsigned char* p) {
    return static_cast<std::uint32_t>(p[0]) | (static_cast<std::uint32_t>(p[1]) << 8) |
           (static_cast<std::uint32_t>(p[2]) << 16) | (static_cast<std::uint32_t>(p[3]) << 24);
}
Audio read_wav(const std::string& path) {
    std::ifstream f(path, std::ios::binary);
    if (!f) throw std::runtime_error("Cannot open WAV: " + path);
    std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(f)), {});
    if (bytes.size() < 44 || std::memcmp(bytes.data(), "RIFF", 4) != 0 ||
        std::memcmp(bytes.data() + 8, "WAVE", 4) != 0) {
        throw std::runtime_error("Expected RIFF/WAVE file");
    }
    std::uint16_t encoding = 0, channels = 0, bit_depth = 0;
    std::uint32_t sample_rate = 0;
    const unsigned char* payload = nullptr;
    std::size_t payload_size = 0;
    for (std::size_t offset = 12; offset + 8 <= bytes.size();) {
        const std::uint32_t n = u32(bytes.data() + offset + 4);
        if (static_cast<std::size_t>(n) > bytes.size() - (offset + 8))
            throw std::runtime_error("Invalid WAV chunk size");
        const auto* data = bytes.data() + offset + 8;
        if (std::memcmp(bytes.data() + offset, "fmt ", 4) == 0) {
            if (n < 16) throw std::runtime_error("Invalid WAV fmt chunk");
            encoding = u16(data);
            channels = u16(data + 2);
            sample_rate = u32(data + 4);
            bit_depth = u16(data + 14);
            // WAVE_FORMAT_EXTENSIBLE: the sub-format's first two bytes
            // hold PCM (1) or IEEE_FLOAT (3).
            if (encoding == 0xfffe && n >= 40) encoding = u16(data + 24);
        } else if (std::memcmp(bytes.data() + offset, "data", 4) == 0) {
            payload = data;
            payload_size = n;
        }
        offset += 8 + static_cast<std::size_t>(n) + (n & 1u);
    }
    if (sample_rate == 0 || channels == 0 || channels > 8 || !payload)
        throw std::runtime_error("WAV is missing valid fmt/data chunks");
    const int bytes_per_sample = bit_depth / 8;
    if (!(encoding == 3 && bit_depth == 32) &&
        !(encoding == 1 && (bit_depth == 16 || bit_depth == 24 || bit_depth == 32))) {
        throw std::runtime_error("WAV must be PCM 16/24/32 or float32");
    }
    if (payload_size % (static_cast<std::size_t>(bytes_per_sample) * channels) != 0)
        throw std::runtime_error("Truncated audio frame in WAV");
    Audio result;
    result.sample_rate = sample_rate;
    result.channels = channels;
    result.interleaved.reserve(payload_size / bytes_per_sample);
    for (std::size_t p = 0; p < payload_size; p += bytes_per_sample) {
        const auto* b = payload + p;
        float value = 0.0f;
        if (encoding == 3) {
            const auto bits = u32(b);
            std::memcpy(&value, &bits, sizeof(value));
        } else if (bit_depth == 16) {
            value = static_cast<float>(static_cast<std::int16_t>(u16(b))) / 32768.0f;
        } else if (bit_depth == 24) {
            std::int32_t sample = static_cast<std::int32_t>(b[0] | (b[1] << 8) | (b[2] << 16));
            if (sample & 0x800000) sample |= ~0xffffff;
            value = static_cast<float>(sample) / 8388608.0f;
        } else {
            value = static_cast<float>(static_cast<std::int32_t>(u32(b))) / 2147483648.0f;
        }
        result.interleaved.push_back(value);
    }
    return result;
}

std::vector<Region> read_regions(const std::string& path) {
    std::ifstream f(path);
    if (!f) throw std::runtime_error("Cannot open region CSV: " + path);
    std::vector<Region> result;
    std::string line;
    while (std::getline(f, line)) {
        if (line.empty() || line[0] == '#') continue;
        std::replace(line.begin(), line.end(), ',', ' ');
        std::istringstream parser(line);
        Region r;
        if (!(parser >> r.name >> r.begin >> r.end >> r.expected_hz) ||
            r.begin < 0.0 || r.end <= r.begin || r.expected_hz <= 0.0)
            throw std::runtime_error("Invalid region CSV row: " + line);
        result.push_back(r);
    }
    return result;
}

double median(std::vector<double> values) {
    if (values.empty()) return 0.0;
    std::sort(values.begin(), values.end());
    const auto mid = values.size() / 2;
    return values.size() % 2 ? values[mid] : 0.5 * (values[mid - 1] + values[mid]);
}

void report(const std::vector<Row>& rows, const std::vector<Region>& regions) {
    int pass = 0;
    for (const auto& region : regions) {
        std::vector<double> pitch, cents;
        std::size_t total = 0, valid = 0, correct = 0;
        for (const auto& row : rows) {
            // Ignore attacks; the YIN window needs a few periods to settle.
            if (row.seconds < region.begin + 0.25 || row.seconds >= region.end) continue;
            ++total;
            if (!row.gate || !row.valid || row.hz <= 0.0f) continue;
            ++valid;
            pitch.push_back(row.hz);
            const double error = 1200.0 * std::log2(row.hz / region.expected_hz);
            cents.push_back(error);
            if (std::abs(error) <= 60.0) ++correct;
        }
        const double valid_pct = total ? (100.0 * valid / total) : 0.0;
        const double correct_pct = total ? (100.0 * correct / total) : 0.0;
        const double median_pitch = median(pitch);
        const double median_cents = median(cents);
        const bool ok = total > 0 && valid_pct >= 70.0 && correct_pct >= 70.0 &&
                        std::abs(median_cents) <= 45.0;
        pass += ok;
        std::cout << (ok ? "PASS " : "FAIL ") << region.name << " expected "
                  << std::fixed << std::setprecision(2) << region.expected_hz
                  << " Hz; median " << median_pitch << " Hz; median error "
                  << median_cents << " cents; valid " << std::setprecision(1)
                  << valid_pct << "%; correct " << correct_pct << "%\n";
    }
    std::cout << "Regions passing: " << pass << "/" << regions.size() << "\n";
    if (pass != static_cast<int>(regions.size())) throw std::runtime_error("Pitch-regression test failed");
}
} // namespace

int main(int argc, char** argv) {
    try {
        std::string wav, regions, csv;
        for (int i = 1; i < argc; ++i) {
            const std::string flag = argv[i];
            if ((flag == "--wav" || flag == "--regions" || flag == "--csv") && i + 1 < argc) {
                const std::string value = argv[++i];
                if (flag == "--wav") wav = value;
                else if (flag == "--regions") regions = value;
                else csv = value;
            } else throw std::runtime_error("Unknown/missing argument: " + flag);
        }
        if (wav.empty()) throw std::runtime_error("Usage: yin_offline --wav FILE [--regions regions.csv] [--csv trace.csv]");
        const auto audio = read_wav(wav);
        std::cout << "Input: " << audio.sample_rate << " Hz, " << audio.channels
                  << " channel(s), " << audio.interleaved.size() / audio.channels << " frames\n";
        wyldtone::YinTracker tracker(static_cast<float>(audio.sample_rate));
        std::vector<Row> rows;
        const std::size_t frames = audio.interleaved.size() / audio.channels;
        rows.reserve(frames / 128 + 1);
        for (std::size_t frame = 0; frame < frames; ++frame) {
            float mono = 0.0f;
            for (std::size_t ch = 0; ch < audio.channels; ++ch)
                mono += audio.interleaved[frame * audio.channels + ch];
            mono /= static_cast<float>(audio.channels);
            const auto estimate = tracker.process_sample(mono);
            if (frame % 128 == 127) {
                rows.push_back({static_cast<double>(frame) / audio.sample_rate,
                                estimate.hz, estimate.confidence, estimate.gate, estimate.valid});
            }
        }
        if (!csv.empty()) {
            std::ofstream out(csv);
            if (!out) throw std::runtime_error("Could not create CSV: " + csv);
            out << "time_seconds,pitch_hz,confidence,gate,valid\n";
            for (const auto& row : rows) {
                out << std::fixed << std::setprecision(6) << row.seconds << ','
                    << row.hz << ',' << row.confidence << ','
                    << (row.gate ? 1 : 0) << ',' << (row.valid ? 1 : 0) << '\n';
            }
            std::cout << "Trace: " << csv << '\n';
        }
        if (!regions.empty()) report(rows, read_regions(regions));
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << '\n';
        return 1;
    }
}
