# SDK beta code review — 2026-11-06

This is a **focused manual review**, not an exhaustive source-code, legal
provenance, fuzzing or security audit. In particular, it cannot prove that
AI-assisted code is original or not derived from uncredited copyrighted work.

## Scope

Inspected core ZIP/WAP loader, manifest conversion, Wasmtime ABI/runtime,
minimal offline WAP host, SDK CLI, JUCE adapter and build scripts, published
license notices, plus the separate JUCE source comparison in
[license-audit-beta.md](license-audit-beta.md). The goal was obvious beta issues,
not production multi-tenant WAP processing or a formal third-party code audit.

## Confirmed findings and changes in PR #3

1. **Unbounded standalone manifest read (fixed):** Unlike the 1 MiB limit
   in `load_wap()`, `load_wap_manifest()` previously used an unbounded
   `read_to_string()` for the ZIP member. It now checks the declared size and
   caps decompressed reads at 1 MiB + 1 byte.
2. **Negative WASM linear-memory offsets (guarded):** The runtime's string,
   choice, plugin-name and audio helpers previously cast signed plugin-returned
   pointers to `usize` without consistently rejecting negative values. The
   reviewed helpers now reject negative offsets before memory addressing.
3. **Personal release notice (updated):** `NOTICE` names Dominik Rogosch as
   copyright holder **only for SDK sources where he holds the necessary rights**;
   no claim is made on JUCE or third-party code.
4. **JUCE compatibility source overlap (fixed separately):** The old
   `ColourCompat.cpp` contained a close match to JUCE's ARGB constructor.
   This was rewritten to use JUCE's public `PixelARGB::setARGB()` API.
   The CI fetches JUCE separately at the documented revision for building
   and rendering a test WAP, and does not upload linked binaries.

## Remaining risks and recommended next work

- `WasmPlugin::reset()` uses `expect()` when a plugin traps; a malformed or
  malicious plugin can cause a host-level panic. Prefer a fallible
  `try_reset() -> Result` API and explicitly propagate failures; preserve
  existing callers until migration. This does not demonstrate sandbox escape.
- Optional WASM ABI exports are read with `.ok()`; an exported function with an
  unexpected type can be treated as absent. For versioned extensions, reject
  an exported-but-malformed function rather than silently falling back.
- The runtime caches compiled modules under the process current directory and
  has no explicit instruction/fuel or epoch budget. WASM sandboxing alone does
  not guarantee execution time or memory bounds. The reference host remains
  **for trusted offline/local plugins**, not a public file-processing service.
- The WAP ZIP loader does not enforce a single canonical inventory of ZIP
  paths, duplicate entries or all unexpected entries. The signature protocol
  explicitly needs these checks *before* package-signature verification and
  any untrusted marketplace ingest.
- The manifest conversion retains an internal `panic!` for logically
  inconsistent parameter metadata. Currently treated as an internal invariant,
  but a library API should prefer a structured error if untrusted metadata can
  reach it.
- License declarations are not proof of code provenance. The sole-author
  statement supports release planning, but full code ownership, external
  fragments, training data, and historic source copying cannot be established
  conclusively from a repository scan.

## Beta triage

For a **source-only SDK beta** whose offline reference host is documented for
trusted local plugins, the fixed bounds/pointer issues and the public JUCE
comparison are worthwhile immediate improvements. The remaining host hardening
and arbitrary untrusted plugins belong to a later security milestone, **before**
marketplace server-side execution, rather than being represented as already
production-hardened.

No change to public WAP licensing ABI or DRM is introduced by this audit.
