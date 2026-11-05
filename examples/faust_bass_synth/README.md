# Wyldtone Bass Synth v0.4 - YIN string-switch reacquisition

An original, **monophonic** bass-controlled synth built with a C++ YIN tracker and a Faust synthesis engine. The update targets a common bass-playing case: a lower string keeps ringing while the player plucks a higher string. It is not polyphonic pitch tracking and is not an emulation of a particular commercial pedal.

## Install and build

Copy the files from this update into your existing Wyldtone SDK tree, preserving their paths. No older v0.3 YIN package needs to be installed separately **if v0.3 is already working**. The update contains the full changed tracker sources and tests, not compiled artifacts.

```bash
sudo docker compose run --rm sdk build \
  examples/faust_bass_synth/plugin.config.json --skip-dsp-test
```

The WAP will be produced in `examples/faust_bass_synth/build/wyldtone/`. You do **not** need to rebuild the Docker image: the changed source files are mounted into the existing SDK container.

Keep `--skip-dsp-test` for this first build. The generic parameter probe is not a reliable test of musical pitch tracking; use the included audio-specific C++ tests and then try your bass.

## What changed

The C++ detector now has **two complementary analysis paths**:

1. **Normal YIN:** Follows the fundamental of a single bass note with the existing 6 kHz, low-pass-filtered analysis stream.
2. **New-onset/reacquisition path:** Compares the input with a copy delayed by one period of the *previously accepted* bass note. A stable old note mostly cancels out. A newly plucked string remains in this *novelty/residual signal*; a second, shorter YIN analysis estimates its pitch. It is accepted only after consecutive consistent high-confidence estimates. The original note is retained if a replacement cannot be verified.

Once a new note is acquired, the accepted pitch is temporarily protected against falling back to the stronger, still-ringing old note. This protection can also delay intentional pitch bends or slides. The prior gate-release reset after silence remains in place. Neither the Faust adapter nor the externally visible stereo (2-in/2-out) ABI has changed.

This is **not** complete multi-pitch detection. It targets a new pluck over a ringing previous note, and it cannot reliably separate chords, fast repeated plucks with dense overlap, or notes whose harmonic content strongly overlaps.

## Controls

All v0.3 sound controls are retained. New:

- **Tracking Response (25-105 ms, default 55 ms):** adjusts how many successive YIN reacquisition observations are required: lower values accept changes more readily and may glitch; higher values are more conservative and can feel slower. The control also changes the note-gate smoothing slightly (~1.25-5.25 ms) so it participates in Faust's DSP parameter tree. The displayed number is a tuning control **not a guarantee of total latency**: actual onset detection and YIN analysis require a larger window (~70-100 ms or more).

Initial recommendation: **55 ms (Balanced)**. For fast playing try 25-40 ms; for unstable tracking try 80-105 ms. Keep Synth Mix at 100%, Glide low, Sub Octave 0%, Stereo Width 0% when assessing pitch accuracy.

All other controls, such as `Tracking Source` and `Tracking Octave`, continue to work. The two obsolete v0.2 controls `Tracking Lowpass` and `Tracking Response` from its prior zero-crossing implementation were already removed in v0.3; this v0.4 control is newly implemented and has different semantics. Recheck older preset values if relevant. The plugin package version is now `0.4.0`.

## Automated tests

The C++ tests are optional when *building* the plugin, but worth running before publishing:

```bash
sudo docker compose run --rm sdk shell -lc '
  cmake -S examples/faust_bass_synth/tracking \
    -B /tmp/wyldtone-yin -DCMAKE_BUILD_TYPE=Release &&
  cmake --build /tmp/wyldtone-yin --parallel 1 &&
  ctest --test-dir /tmp/wyldtone-yin --output-on-failure
'
```

The test suite checks:

- `YinTrackerSynthetic`: E1, A1, D2, G2 at 44.1/48/96 kHz, separate notes.
- `BassSynthControlIntegration`: pitch/gate on the two *internal* Faust control channels, and proper reset after silence.
- `YinTrackerLegatoSwitch`: E1->A1, A1->D2, D2->G2, G2->E1 with overlapping old and new notes, including a **quieter incoming note**, plus sustained-note stability checks at all three rates.

Local native results: all three CTest cases pass. On generated harmonic bass signals, detection of the newly plucked note occurred about 84-96 ms after the new onset at 44.1/48/96 kHz. In additional *locally prepared overlaps* of the user's E/A/D/G dry recording, the change was detected about 89-173 ms after the new onset, depending on the pair. These were **offline algorithm tests**, not a Faust/WASM/audio-interface latency measurement; the overlapping dry clips are not included in the public SDK.

## Limitations and trade-offs

- **Monophonic only.** If two strings are deliberately played as a chord, exactly one pitch is still emitted.
- **Rapid repeated legato transitions** (especially several plucks <200-250 ms apart with multiple strings still ringing) may not be detected reliably. Muting unused strings still helps.
- **Slides/bends:** after accepting a plucked note, a temporary protection interval (~450 ms) avoids dropping back to a loud old string. During that interval large bends or slides without a new pluck can lag.
- **Intrinsic detection time:** YIN needs adequate history. The `Tracking Response` slider does not eliminate the analysis-window delay.
- **No realtime guarantees from native tests:** the target Faust/Emscripten WASM build, AudioGraph audio stability, and CPU budget must be verified on the user's host.
- This is not a transient-based granular resynthesizer, nor a multi-voice synth. True polyphony would require multi-pitch estimation and separate voice management.

For A/B, keep a copy of your working v0.3 WAP before installing the new v0.4 version. Test clean DI E-A-D-G first; then play normal overlapping string changes, varying Tracking Response and noting the trade-off between glitches and speed.

## Licensing

The changed example DSP and tracking files are Apache-2.0. Faust compiler/library and other SDK dependencies retain their own respective licenses.
