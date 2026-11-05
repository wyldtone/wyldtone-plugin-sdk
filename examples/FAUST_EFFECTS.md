# Faust effects for Wyldtone SDK

These four **original, editable Faust `.dsp` examples** are meant to be dropped
into the SDK `examples/` directory. They reuse the already-installed Faust
v0.5 backend: no engine changes, patches to JUCE, or new dependencies are
required beyond Faust's standard libraries.

Each example defines a **single stereo input bus and stereo output bus**:
`"audio": {"inputs": [2], "outputs": [2]}`.

## Build on wyserver

From the SDK root (on the host):

```bash
sudo docker compose run --rm sdk build examples/faust_parametric_eq/plugin.config.json
sudo docker compose run --rm sdk build examples/faust_bass_mid_drive/plugin.config.json
sudo docker compose run --rm sdk build examples/faust_stereo_chorus/plugin.config.json
sudo docker compose run --rm sdk build examples/faust_stereo_slapback/plugin.config.json
```

The normal `build` command performs manifest generation, DSP-parameter tests,
and packaging. The `.wap` appears under that example's `build/wyldtone/` folder.
`--skip-dsp-test` is useful for the first build if a test exposes an error.

## The four examples

### 1. `faust_parametric_eq` — Wyldtone Parametric EQ

Stereo, four independent frequency bands:

- Low shelf: frequency (25–400 Hz), gain (±15 dB)
- Low mid: frequency (80–2000 Hz), gain (±15 dB), Q (0.3–8)
- High mid: frequency (400–8000 Hz), gain (±15 dB), Q (0.3–8)
- High shelf: frequency (1500–12000 Hz), gain (±15 dB)
- Output gain (−18 to +12 dB)

Uses Faust `fi.low_shelf`, `fi.peak_eq_cq`, and `fi.high_shelf`.
At defaults, all EQ gains are zero, so changes to frequency/Q alone may have no
audible effect. The contextual **Balanced/Deep** tests should vary the matching
gain control, but a warning may remain if the context search budget is exhausted.
Use this as an example of **context-dependent parameter tests**, not a defect.
Large positive boosts can clip downstream: lower output gain as needed.

### 2. `faust_bass_mid_drive` — Wyldtone Bass Mid Drive

Stereo bass overdrive with a **clean low-frequency path**. Each channel passes
through a fourth-order Linkwitz–Riley crossover (`fi.crossover2LR4`). The low
band remains clean and is recombined with a parallel high-band mix between
clean upper band and a `tanh` waveshaper. Controls: crossover, drive, tone,
high-band drive blend, and output gain.

This is intentionally **not an oversampled circuit model**. Strong saturation
may alias at high harmonics; for production a selectable oversampling stage or
anti-aliasing approach should be added. Splitting/recombining bands can change
phase and transient behavior even when high-band drive blend is zero; check
with real bass recordings. The implementation is a starting point for
pedal-style sounds, not a mastering-quality transparent effect.

### 3. `faust_stereo_chorus` — Wyldtone Stereo Chorus

True stereo chorus using two fractional delay lines (`de.fdelay`) modulated by
independent, phase-offset sine LFOs. The width control varies the phase
offset between left and right. Controls: rate, base delay, depth, stereo width,
and mix. At `Depth = 0`, LFO rate and width do not affect audio; this is an
**expected parameter dependency**.

Changing delay time in real time modulates pitch by design. The chorus has no
feedback control (by design), so it is a simple, stable introduction to
modulated delays. At extreme wet mix settings the original transient may be
partially attenuated.

### 4. `faust_stereo_slapback` — Wyldtone Stereo Slapback

Single-repeat stereo delay with variable L/R time offset, low-pass tone and
wet/dry mix. Uses `de.sdelay` to crossfade between changed delay times.
Controls: delay time, stereo spread, tone, mix. This is not a tape echo and
has **no feedback repeats**; the simple architecture is intended for teaching.
It maintains independent stereo channels.

With a very short DSP probe, delayed audio might not arrive before the test
ends. The current SDK probe processes 64 × 256 frames at 48 kHz by default
(~341 ms), and contextual probes are shorter (~128 ms). This may generate
`WARN: no_observable_change` even though the effect works normally. For
verification, use a looped bass/guitar recording or a longer impulse-response
test; this is a known limit of the current generic DSP probe.

## DSP and runtime considerations

- All controls are discoverable automatically by Wyldtone's Faust ABI bridge;
  you do **not** have to write an explicit parameter manifest.
- Stable parameter IDs derive from Faust widget/group labels. Changing labels
  in a released plugin can invalidate saved automation/presets.
- Controls are smoothed with `si.smoo` to avoid zipper noise; after
  parameter changes the audible effect is not instantaneous.
- Faust DSP instances keep **separate state per channel**. Neither of the
  time-based examples creates a separate sidechain bus.
- The existing Faust bridge limits one process call to 2048 frames, and has
  a shared WASM audio buffer; keep the limits in `docs/faust.md` in mind.
- Effect processing time and phase response must be checked in the host. These
  source examples have **not been end-to-end compiled against your Docker
  image in this delivery**; run the SDK build commands above to verify.

## Run explicit tests and benchmark

```bash
sudo docker compose run --rm sdk test-dsp \
  examples/faust_parametric_eq/build/wyldtone/plugin.wasm --profile balanced

sudo docker compose run --rm sdk test-dsp \
  examples/faust_bass_mid_drive/build/wyldtone/plugin.wasm --profile deep

sudo docker compose run --rm sdk benchmark \
  examples/faust_stereo_chorus/build/wyldtone/plugin.wasm \
  --reference-id wyserver-v1
```

Performance scores on `wyserver-v1` are reference measurements, not a guarantee
of real-time performance on a different CPU. Use the same scenario and
sample-rate when comparing plugins.

## License and provenance

The four `.dsp` files were authored for Wyldtone as example implementations;
no third-party plugin source files are copied. They contain an Apache-2.0 SPDX
identifier. When distributing built output, still review licenses and notices
for the exact Faust compiler, standard libraries, architecture, and toolchain
versions used. See `docs/faust.md` in the main SDK.
