#!/usr/bin/env python3
"""Check rendered C++ and Faust gain WAPs against their declared dB mapping.

CI creates three float32 stereo WAVs per example with the same test tone:
low (-24 dB), unity (0 dB) and high (+12 dB). Only Python stdlib is used.
"""
import math
from pathlib import Path
import struct
import sys

EXPECTED_RMS_UNITY = 0.25 / math.sqrt(2.0)
TOLERANCE = 0.025


def read_float32_stereo(path):
    data = Path(path).read_bytes()
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise AssertionError(f"{path}: not a RIFF/WAVE")
    offset = 12
    fmt = None
    payload = None
    while offset + 8 <= len(data):
        kind, size = struct.unpack_from("<4sI", data, offset)
        start = offset + 8
        end = start + size
        if end > len(data):
            raise AssertionError(f"{path}: truncated WAV")
        if kind == b"fmt ":
            fmt = data[start:end]
        if kind == b"data":
            payload = data[start:end]
        offset = end + (size % 2)
    if fmt is None or payload is None or len(fmt) < 16:
        raise AssertionError(f"{path}: missing fmt/data")
    codec, channels, rate = struct.unpack_from("<HHI", fmt, 0)
    bits = struct.unpack_from("<H", fmt, 14)[0]
    if (codec, channels, rate, bits) != (3, 2, 48000, 32):
        raise AssertionError(f"{path}: unexpected format {(codec, channels, rate, bits)}")
    if len(payload) != 48000 * 2 * 4:
        raise AssertionError(f"{path}: expected exactly one second of stereo audio")
    sum_sq = [0.0, 0.0]
    for i, (sample,) in enumerate(struct.iter_unpack("<f", payload)):
        if not math.isfinite(sample):
            raise AssertionError(f"{path}: NaN/Inf sample")
        sum_sq[i % 2] += sample * sample
    return tuple(math.sqrt(s / 48000) for s in sum_sq)


def verify(folder):
    rms = {}
    for name in ("low", "unity", "high"):
        rms[name] = read_float32_stereo(folder / f"ci-{name}.wav")
        print(f"{folder.name} {name}: L/R RMS {rms[name][0]:.6f}/{rms[name][1]:.6f}")
    for channel in range(2):
        baseline = rms["unity"][channel]
        if baseline <= 0:
            raise AssertionError("Unity signal is silent")
        if not math.isclose(baseline, EXPECTED_RMS_UNITY, rel_tol=TOLERANCE):
            raise AssertionError(f"Unity gain wrong: {baseline:.6f}")
        for name, db in (("low", -24.0), ("high", 12.0)):
            ratio = rms[name][channel] / baseline
            target = 10.0 ** (db / 20.0)
            if not math.isclose(ratio, target, rel_tol=TOLERANCE):
                raise AssertionError(
                    f"{folder.name} {name} ch{channel}: gain {ratio:.5f}, expected {target:.5f}"
                )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage: verify_gain_render.py DIR_WITH_ci-low/unity/high.wav")
    verify(Path(sys.argv[1]))
    print("PASS: stereo samples and normalized gain parameter mapping")
