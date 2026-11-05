# SDK crates

- `wyldtone-plugin-api`: manifest, plugin config, parameter metadata
- `wyldtone-plugin-runtime`: Wasmtime-backed public WAP ABI implementation
- `wyldtone-plugin-packager`: `.wap` packaging and icon validation
- `wyldtone-cli`: C++/JUCE/Faust build/test/benchmark CLI
- `wyldtone-wap-host`: minimal **offline** WAP-to-WAV reference renderer

This workspace does not include or depend on the proprietary audio engine.
