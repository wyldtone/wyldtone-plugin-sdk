# Public beta readiness — source-only Wyldtone Plugin SDK

Scope: publish original SDK source, adapters, templates, examples, Dockerfile
and documentation under the applicable license. **Do not distribute** a
prebuilt Docker toolchain image, JUCE checkout, JUCE-linked WAP files or
temporary modified JUCE sources as part of this beta.

## Verified in repository and GitHub Actions

- [x] Public tree excludes target/build caches, private plugin fixtures,
      generated JUCE sources and prebuilt WAP/WAV binaries.
- [x] JUCE framework is obtained separately; source overlap in
      `ColourCompat.cpp` was replaced and reviewed against the public
      baseline. No JUCE-linked binaries are published.
- [x] Original JUCE gain example builds, is inspected and renders a non-silent
      WAV in the dedicated CI workflow with pinned external JUCE checkout.
- [x] `cargo test --release --locked --workspace` runs in CI through the
      Docker `sdk test` launcher; latest SDK smoke workflow succeeded.
- [x] Vanilla C++ `examples/gain` and `examples/faust_gain` compile,
      and both have automated sample-level stereo gain regression tests in CI.
- [x] SPDX declarations inventoried for 281 external resolved Rust packages:
      276 marked permissive; five reviewed for permissive alternative or
      bzip2 license. See `docs/license-audit-beta.md` and the CI artifact.
- [x] Source-level focused code review performed, findings documented and
      manifest-read/pointer guard fixes incorporated in PR #3. See
      `docs/code-review-beta.md`.
- [x] Personal copyright notice identifies Dominik Rogosch, scoped to original
      SDK material for which he holds rights; independent third-party notices
      are preserved and not relicensed.

## Remaining before beta publication

- [x] Reviewed Faust's exact architecture header licenses
      (`dsp.h`, `UI.h`, `meta.h`): GRAME copyright, LGPL-2.1-or-later
      with special exception for unchanged architecture sections.
      `examples/faust_gain` imports no standard library, and this source-only
      release distributes no Faust-generated C++ or compiled Faust WAP.
      See `docs/license-audit-beta.md`.
- [x] Clean CI renders both C++ and Faust WAPs at -24 dB, unity and
      +12 dB; validates float32 stereo WAV data and RMS gain mapping per
      channel against expected decibel ratios. Verified in SDK smoke workflow
      run 42205164010.
- [x] Inspected Git-tracked beta source inventory, available main
      commit history, current GitHub Releases and representative workflow
      artifacts. No generated JUCE/Faust/WAP binaries, vendored JUCE sources,
      or published Docker images in the inspected source tree/release assets.
      Added CI index guard and extended ignores. See
      `docs/release-inventory-audit.md`. **Owner should explicitly confirm
      that the full bass synth/YIN and loop-freeze example sources are intended
      to remain public**; these are intentionally tracked examples, not builds.
- [ ] Review PR #3, merge it to `main`, confirm required workflows succeed
      on release revision, and publish/tag the chosen beta version.

## Confirmed release-owner conditions (not automation assertions)

- Dominik Rogosch is the initial personal publisher and states he is the sole
  creator of Wyldtone. The Apache-2.0 grant applies only to code he has the
  rights to distribute; AI assistance does not itself prove provenance or
  dispose of rights in possible external fragments.

## Separate milestone: marketplace / untrusted plugins / commercial distribution

- [ ] Harden execution of untrusted plugins with time/memory budgets and
      isolated processes; improve runtime trap/ABI error handling.
- [ ] Audit canonical WAP ZIP contents, duplicates and signatures before
      server-side marketplace inspection or acceptance of untrusted uploads.
- [ ] Obtain appropriately scoped advice about JUCE AGPL/commercial licensing
      and the proprietary host/marketplace model before relevant distribution.
- [ ] Produce a comprehensive system-package / container-image SBOM and
      licensing/notice bundle if the SDK Docker image is redistributed.
- [ ] Implement and review any optional commercial licensing/DRM components
      before enabling those features.

The GitHub Action's JUCE test is ephemeral: it fetches JUCE independently,
builds only for regression testing and uploads no JUCE binary artifact.
This checklist records engineering verification, not a definitive legal
ownership opinion or security certification.
