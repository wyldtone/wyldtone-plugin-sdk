# Public SDK vs proprietary product boundary

This repository contains the public WAP manifest/ABI, plugin build adapters,
packager, offline DSP probes/benchmarks, and a minimal independent offline
renderer (`crates/wyldtone-wap-host`). It does not contain the Wyldtone audio
engine, audio graph, live input/output, latency compensation, NAM workers, UI,
accounts or marketplace backend.

The reference host is deliberately simple and processes one mono/stereo stream
from a WAV or generated sine wave entirely offline. There is no live audio
path, device enumeration, host graph or hidden engine API.

See `docs/wap-format.md`, `docs/reference-host.md` and `docs/juce-licensing.md`.
