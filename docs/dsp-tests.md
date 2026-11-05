# Wyldtone SDK — DSP parameter tests

This page explains **what the automatic DSP probe tests, what each warning means, and how to investigate it**. The same test host is used for Vanilla C++ and headless JUCE plugins. The test runs on the **compiled `plugin.wasm`**, not on the original JUCE editor or the host application's audio graph.

> **Important:** `PASS` means only that a parameter produced a measurable output difference in at least one tested situation. `WARN` means the test could not establish that effect. Neither is a statement about sound quality, real-time safety, parameter correctness, or full coverage of every mode.

## Quick start (Docker build host)

```bash
# Default: balanced DSP checks as part of a normal plugin build
sudo docker compose run --rm sdk build examples/gain/plugin.config.json

# Quick smoke test during development
sudo docker compose run --rm sdk build examples/gain/plugin.config.json --dsp-profile quick

# Extra contexts when investigating warnings (does not rebuild the plugin)
sudo docker compose run --rm sdk test-dsp \
  examples/faust_parametric_eq/build/wyldtone/plugin.wasm \
  --profile deep

# Strict mode for a project whose parameters are ALL expected to affect audio
sudo docker compose run --rm sdk test-dsp \
  examples/gain/build/wyldtone/plugin.wasm --profile balanced --fail-on-inactive

# Deliberately skip the probe (e.g. an incompatible bus layout)
sudo docker compose run --rm sdk build examples/gain/plugin.config.json --skip-dsp-test
```

CLI usage outside Docker:

```
wyldtone build <plugin.config.json> [--dsp-profile quick|balanced|deep] [--skip-dsp-test | --fail-on-inactive]
wyldtone test-dsp <plugin.wasm> [--profile quick|balanced|deep] [--fail-on-inactive]
```

`--dsp-profile` applies to `build`; `--profile` applies to `test-dsp`. Defaults to **balanced** in both CLI commands. Tests generate `build/wyldtone/dsp-test-report.json` beside the compiled WASM. A build normally still creates the `.wap` when the probe produces only parameter warnings.

## Profiles and scope

| Profile | What is tested | Maximum extra contexts | Intended use |
| --- | --- | --- | --- |
| **quick** | Every parameter, with other controls at defaults | 0 | Fast smoke tests |
| **balanced** (default) | Quick, then vary **one other control at a time** for still-inactive parameters | 12 per parameter, 48 for the plugin | Ordinary builds / CI |
| **deep** | Balanced-style contexts, then combinations of **two other controls** for still-inactive parameters | 42 per parameter, 120 for the plugin | Investigating uncertain results |

The baseline for **every** parameter renders 64 blocks × 256 frames at 48 kHz (about 341 ms) with six stimuli. Each additional context renders 24 blocks × 256 frames (about 128 ms) using three stimuli. These are **audio durations**, not a promise about CPU or wall-clock time; complex effects, WASM instantiation and two-core build hosts may take much longer.

Test inputs are: 110 Hz sine, 440 Hz sine, 2 kHz sine, quiet 440 Hz sine, two simultaneous sine tones, and seeded deterministic noise. Context tests use noise, quiet sine and two-tone. One compatible mono or stereo input/output bus is supported. Instruments, sidechains or multiple buses are reported as `SKIP` rather than being incorrectly judged broken.

### Why not test every combination?

With 17 parameters and three values each, an exhaustive sweep would require **129,140,163 configurations** *before* testing different audio signals. The SDK therefore samples a **bounded subset**, prioritizing `Choice`, `Bool`, and integer mode controls before floating-point controls.

For each parameter under test, the test holds all **context** parameters constant, renders the target at normalized **0.0 vs 1.0** in separate, freshly initialized WASM instances, and compares those outputs. It never compares two different contexts as if they were a target-parameter change.

- For boolean controls the context values are off/on.
- For a choice with up to five items, all items are considered; larger choices sample their beginning, middle, and end.
- Other context controls are sampled at normalized 0, 0.5 and 1 (excluding their default when redundant).
- The first context where a target's audio effect is measurable is recorded in the report. The profile prioritizes finding **one** witness, not proving behavior in all contexts.
- **This is not a full pairwise covering array**, and it can miss three-way interactions, very narrow parameter ranges, delayed behavior, or effects that require a special input.

An output difference counts as observable only if **both** of these criteria are exceeded:

- Absolute RMS sample difference > `0.00002`.
- Relative RMS difference > `0.002` (0.2% relative to the louder output RMS, with a small numerical floor).

These thresholds are heuristics, not perceptual loudness criteria. A large relative difference with an extremely tiny absolute difference still does not count. The probe also repeats the best case with **identical** parameter values to spot nondeterministic audio.

## Reading PASS, WARN and SKIP

| Console / JSON verdict | Meaning | What to do |
| --- | --- | --- |
| `PASS: ... changed` | A parameter produced a measurable audio difference on at least one stimulus/context | Inspect `best_signal` and `best_context`; this does **not** verify that the effect is musically or numerically correct |
| `WARN: ... no_observable_change` | No sample difference passed both thresholds in the tested contexts | Check activation modes, other controls, DSP/GUI separation, test signal, duration, offline-only semantics |
| `WARN: ... parameter_not_updated` | The normalized value read back from the WASM parameter getter did not change between the tested endpoints | Check parameter export, read/write mapping, clamp/rounding, non-automatable or coupled controls |
| `WARN: ... inconclusive_nondeterministic` | Identical settings on fresh instances produced differences that confound the measured change | Check randomness, unseeded noise, asynchronous states, warm-up, and phase; repeat under a deterministic test mode |
| `WARN: ... context budget reached` | The configured cap prevented further candidate contexts from being examined | Re-run `--profile deep`, and examine the report before interpreting other `WARN`s |
| `SKIP: ...` | No meaningful mono/stereo audio probe could be performed for this bus/parameter structure | Inspect `skipped_reason`. Add a plugin-specific host test if necessary |
| Build error, WASM trap, NaN/Inf | The plugin or test host could not run safely under the tested conditions | Investigate the actual runtime error; **not** merely an inactive-parameter warning |

**`--fail-on-inactive` is intentionally opt-in**: it exits nonzero for any non-`changed` parameter or skipped probe. Do not enable it indiscriminately for all Marketplace plugins. Metadata-only controls, monitoring parameters, rendering settings and UI controls can validly produce no sample difference in an offline real-time DSP probe.

## Investigating a WARN: step-by-step

1. **Open `dsp-test-report.json`**, find the parameter `id`, and note `verdict`, `best_signal`, `best_context`, `tested_contexts`, `low_readback`, `high_readback`, and `context_budget_exhausted`.
2. **Check whether the parameter is expected to change audio immediately.** A GUI-only Trim Lock, rendering-quality option, MIDI routing switch or meter reset may not. Avoid treating such warnings as defects without verifying the intent.
3. **Check dependencies.** If the parameter only works in a particular mode/revision, run `--profile deep`. The test searches contexts automatically; it does **not** know the original developer's intent.
4. **Check JUCE parameter wiring.** DSP should read from the parameter/processor state, **not** rely on the GUI's slider callbacks. The original KFuzz `hz`/Fuzz Shape issue was an example of a truly disconnected parameter.
5. **Check the stimulus and duration.** A compressor threshold may be above every test signal; a gate may always be open; a long delay or slow LFO may not reach the audible region within the test; a subtle EQ may not exceed the thresholds.
6. **Check offline vs real-time behavior.** A `Render Oversampling` parameter may only have an effect in offline/non-real-time playback, which the current probe does not enable.
7. **Reproduce manually.** Load the `.wap` in Wyldtone, test the applicable revision with appropriate audio levels, and monitor CPU, clipping, actual signal changes and state restoration.
8. **Document intentional exceptions** in the plugin's development notes. Keep the warning in the report unless a dedicated functional test has independently verified that behavior. Do not silence warnings by changing arbitrary thresholds globally.

### Example: NoAmp Low Rider DI

- `Mid`, `Mid Shift`, `Bass Shift`: look for a DSP response in **Revision V2**. Balanced tries discrete Revision values early, but the global budget can prevent full coverage.
- `HQ`: likely needs a suitable revision, strong drive and an input level that exercises the nonlinear stage. Deep tries some additional two-control interactions; no effect remains a warning, not a verdict about the original code.
- `Render Oversampling`: may affect offline renders only; the real-time probe cannot certify it.
- `Trim Lock`: may represent GUI/control synchronization rather than an independently audible DSP effect. Verify whether it is implemented in the processor or only in the original editor.

These are **investigation hypotheses**, not automatically inferred hard-coded rules, and the SDK does not contain NoAmp-specific exceptions.

## JSON report reference

A report is written next to the WASM file, usually `build/wyldtone/dsp-test-report.json`:

```json
{
  "plugin": "Example",
  "profile": "balanced",
  "sample_rate": 48000,
  "block_size": 256,
  "channels": 2,
  "skipped_reason": null,
  "contextual_cases": 5,
  "context_budget_exhausted": false,
  "parameters": [
    {
      "id": "mid",
      "name": "Mid",
      "verdict": "changed",
      "best_signal": "seeded_noise",
      "best_context": "revision=V2 (normalized 1.000)",
      "tested_contexts": 3,
      "relative_rms_difference": 0.14,
      "absolute_rms_difference": 0.02,
      "low_readback": 0.0,
      "high_readback": 1.0
    }
  ]
}
```

*Illustrative JSON only, not a captured NoAmp test.* `tested_contexts` includes the default context. `contextual_cases` is the total number of additional parameter-context experiments. `best_context` describes the tested settings of **other** parameters; the target itself is still compared at normalized 0 and 1. `context_budget_exhausted` means that the test did not finish examining candidate contexts under the selected cap; it is not necessarily a defect in the plugin.

## Limitations and future tests

- This tests only **sample-level audio response**, not whether the change is sensible or pleasant. It also cannot guarantee an effect is active across its entire range.
- A plugin could respond differently after more audio time, a transport/tempo event, a MIDI message, a non-default sample rate, an external sidechain, or an offline-render callback.
- NoAmp or another processor can produce a `PASS` and **still be too expensive for real-time use**. DSP-probe results are **not** CPU benchmarks. For example, ~2 ms processing time at 48 kHz / 512 frames uses ~18.75% of a 10.67 ms audio-block budget **for that one plugin**, before other effects and system overhead.
- For reliable release testing, add plugin-specific DSP reference tests or test vectors, a performance budget, state save/load checks, and extra sample rates / block sizes. Consider a separate real-time `benchmark-dsp` command in a later SDK release; do not infer speed from the current probe.

This document should be linked from plugin author guides and included in release checklists. Developers should attach `dsp-test-report.json` and the tested plugin version when reporting a `WARN`.
