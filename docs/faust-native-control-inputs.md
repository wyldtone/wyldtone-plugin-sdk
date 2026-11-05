# Faust + native C++ control-input bridge

The Wyldtone Faust backend can optionally compile plugin-local C++ sources to
produce additional **internal DSP signal inputs**. This is intended for
algorithms such as pitch detectors that are better tested in native C++ while
Faust implements sound generation.

## Configuration

```json
{
  "faust": {
    "source": "bass_synth.dsp",
    "extra_inputs": 2,
    "extra_sources": [
      "tracking/YinTracker.cpp",
      "tracking/BassSynthControl.cpp"
    ]
  },
  "audio": {"inputs": [2], "outputs": [2]}
}
```

The Faust program accepts *four* input signals: its two standard stereo
inputs followed by its two internal controller signals. The host still sees
**two** input channels. All source paths are relative to the plugin directory.
For existing Faust plugins, omitting `extra_inputs`/`extra_sources` preserves
the previous build path. At most four extra inputs are supported in this
version, with eight total Faust inputs at most.

## Required native hook ABI

If `extra_inputs > 0`, the plugin-local compiled sources must define:

```cpp
extern "C" void wyldtone_faust_extra_prepare(int sampleRate);
extern "C" void wyldtone_faust_extra_reset();
extern "C" void wyldtone_faust_extra_parameter(const char* id, float actualValue);
extern "C" void wyldtone_faust_extra_process(
    const float* const* mainInputs, int mainChannels, int frames,
    float* const* extraOutputs, int extraChannels);
```

The first and second hooks are called when the sample rate is initialized and
state is reset. `extra_parameter` receives the **actual** slider value (not
normalized) after parameter updates and initialization. The `extra_process`
hook runs on the real-time audio thread before Faust `compute()` on every block;
it fills preallocated planar buffers for all declared extra input channels.
Never allocate memory, lock, or perform file/network I/O in this callback.

This API is an SDK v0.5 experimental extension, not a stable public ABI.
For native-controller examples see `examples/faust_bass_synth/tracking/`.
