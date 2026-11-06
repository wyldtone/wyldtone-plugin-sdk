# Wyldtone Loop Freeze (Vanilla C++ / WASM)

An Apache-2.0 **example plugin** for the standalone Wyldtone SDK. No JUCE,
Faust, browser threads, filesystem, or engine changes are required.

## What kind of Freeze is this?

This is a **live audio loop freeze**, not a spectral/reverb freeze. While you
play, the plugin stores the last audio in a rolling stereo buffer. When the
**Freeze** parameter changes from Off to On it holds that buffer, loops the
selected part forever, and crossfades the loop seam. The live dry signal
remains independent. Turning Freeze Off fades out the held sound.

The capture can sound rhythmic or flutter slightly, particularly with a short
loop or sustained harmonic material. The release is not an endlessly decaying
reverb; it is a held snippet that is explicitly released.

## Build from a fresh SDK checkout

The complete plugin source is included under `examples/cpp_loop_freeze/`.
From the SDK root, first build the developer image if needed
(`docker compose build sdk`), then:

```bash
./wyldtone build examples/cpp_loop_freeze/plugin.config.json --skip-dsp-test
```

The generated `plugin.wasm`, `manifest.json` and `.wap` live under
`examples/cpp_loop_freeze/build/wyldtone/`.
The generic DSP parameter probe cannot reproduce the capture/hold sequence
required by a freeze effect, so `--skip-dsp-test` is recommended. Run the
included native regression tests below to check capture, looping and release.

The proprietary live Wyldtone host is still in private testing and is not
publicly available. The SDK's offline reference host can render WAV files,
but its static parameter settings do not replace an interactive Freeze
on/off test.

## Controls

| Parameter | Default | Meaning |
| --- | ---: | --- |
| Freeze | Off | Toggle capture/hold on, fade out off |
| Capture Length | 320 ms | Duration of the *last* audio captured at trigger; only takes effect on next capture |
| Seam Crossfade | 35 ms | Overlap between the end and the start of the captured loop; applied at capture |
| Freeze Attack | 65 ms | Fade-in time after capture |
| Freeze Release | 220 ms | Fade-out time after release |
| Dry Level | 100% | Current unprocessed audio gain |
| Freeze Level | 85% | Captured stereo loop gain |
| Freeze Tone | 11000 Hz | 1-pole low-pass for the captured audio |

The internal IDs (`freeze`, `length_ms`, etc.) are intended to remain stable
for future preset compatibility.

## Quick live test

1. Add the plugin to a stereo signal path, with Freeze **Off**.
2. Play a chord or a long bass note for at least ~0.5 s.
3. Set **Freeze On** while it is still ringing. The prior 320 ms of the input
   should continue sounding after you mute the strings.
4. Play another note while frozen: it should be heard through Dry Level but
   must not replace the held loop.
5. Set Freeze **Off** to release the sustained layer. To capture a new note,
   switch it Off then On again after the previous layer has faded out.

Starting texture values: Capture Length 450 ms, Seam Crossfade 50 ms,
Freeze Attack 90 ms, Freeze Release 350 ms, Dry 100%, Freeze Level 60%,
Freeze Tone 6500 Hz.

For a more rhythmic hold: Capture Length 140 ms, Seam Crossfade 20 ms,
Dry 75%, Freeze Level 85%. Listen for repetitions at short lengths.

## Native regression test (optional)

With a normal system C++17 compiler and CMake:

```bash
cmake -S examples/cpp_loop_freeze/tests -B /tmp/wyldtone-freeze-test -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/wyldtone-freeze-test --parallel 1
ctest --test-dir /tmp/wyldtone-freeze-test --output-on-failure
```

Checks stereo capture, persistence while input is silent, seam continuity,
release, and recapture. Native tests use a stub header; the actual SDK build
uses `abi/include/wasm_plugin.h`.

## Design notes and limits

- 2-in/2-out stereo interleaved audio, existing Wyldtone SDK ABI.
- Preallocated 2 x 524,288 float ring buffer (~4 MiB). No dynamic allocation,
  file access or thread creation in `plugin_process`.
- Captured audio is read directly from the *paused* ring; no copy occurs on
  triggering Freeze. Changes to length/crossfade apply at the next capture.
- Up to 900 ms at sample rates through 384 kHz (within buffer size).
- Capture requires some recorded audio. Triggering immediately after plugin
  initialization can produce no hold; toggle Off/On once the ring has filled.
- Freeze audio is ephemeral: closing or resetting the plugin clears the audio.
  The parameter state is restorable, **not** the captured sound.
- On rapid Off/On toggles during the release, an existing held loop can resume;
  wait for the release to finish to capture fresh audio.
- No time stretch, beat sync, reverse, spectral resynthesis, or automatic
  pitch stabilization in v0.1.
- No fixed DSP algorithm latency for the live dry signal. The held part uses
  previously recorded audio by design, and fades in on trigger.
- At very high dry+freeze levels output may clip. Adjust both levels.
- Audio callback is designed to be allocation-free, but real-time performance
  needs to be measured on the target host and buffer sizes.
- Native C++ tests were run successfully in the artifact environment. The Emscripten/Wasmtime/WAP build and real-time behavior should also be
  validated on each intended target host.
