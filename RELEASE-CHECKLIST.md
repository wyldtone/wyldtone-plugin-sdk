# Public-release readiness checklist

## Completed in this staging copy

- [x] No `target/`, build caches, WAP binaries, WAVE files or private plugin fixtures.
- [x] Removed migration snapshots and historical JUCE source patch.
- [x] Kept JUCE framework external; bridge renamed from `legacy` to `bridge`.
- [x] Added minimal offline WAP host (WAV input or sine → processed WAV).
- [x] Replaced outdated main, architecture and JUCE documentation.
- [x] Added Apache 2.0 license proposal, notices, JUCE licensing and security docs.
- [x] Removed private fixture assumptions from basic configuration inventory.

## Blocking steps requiring owner / build-host verification

- [ ] Confirm the releasing entity owns/can license **every** adapter, Rust
      crate, Faust example and migrated engine-derived source file under Apache 2.0.
- [ ] Review `ColourCompat.cpp`, `HeadlessEditorCompat.h`, VFS transform context,
      and third-party contributions for close copying / compatibility issues.
- [ ] Verify external JUCE's actual checkout, version and licensing;
      confirm no JUCE source gets committed in generated folders.
- [ ] Run `cargo test --release --locked --workspace` in a clean Docker checkout.
- [ ] Build and render examples/gain; verify sample-level gain correctness.
- [ ] Build and render examples/faust_gain; check parameter mapping.
- [ ] Build and render examples/juce_gain with external JUCE checkout.
- [ ] Check Faust compiler/library/architecture licences and source notices.
- [ ] Generate dependency SBOM and third-party licence report (Rust, Docker,
      Emscripten, Faust, JUCE) and review it.
- [ ] Audit security limits before executing *untrusted* marketplace plugins.
- [ ] Have appropriate open-source license counsel review AGPL/host combination
      before marketplace/public commercial distribution.
- [ ] Replace proposed ownership lines in NOTICE with approved legal owner.

This package is a **foundation for publication**, not a claim that all checks
have passed. No proprietary engine source is included.
