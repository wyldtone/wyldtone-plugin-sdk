# Source-only public beta inventory audit — 2026-11-06

## Inspected

- GitHub Git Trees for `main` (`ec8e64aae70b0f3feb0f002167237b3d4b6b381e`)
  and PR #3 head before audit changes
  (`cfaeeff83741081be97fcadcd857a83ddc2e206a`).
  Both tree API responses had `truncated: false`.
- `main`'s 19 commits at the time of the audit: no deleted or later-removed
  executable/WAP/WAV/secret files observed in their changed-file lists.
- The initial public commit contained 111 tracked files; no initial-commit
  paths disappeared by the inspected `main` revision.
- GitHub Releases API returned **zero releases**. Workflow artifact listings
  examined contained Docker Buildx **build records** and the Rust license
  inventory, **not** JUCE/Faust/WAP binaries. Buildx build records are not
  distributable Docker images.
- Source-only Git inventories showed no tracked binaries with `.wap`,
  `.wasm`, `.wav`, `.nam`, `.exe`, `.dll` or `.zip` extensions;
  nor a vendored JUCE checkout or the compiler toolchain.
- `.gitignore`, `.dockerignore`, `compose.yaml`, Dockerfile and reference
  host/example package tests were inspected.

## Source-code examples intentionally included (owner confirmed)

This repo includes **more than the three basic gain examples**. In particular:

- `examples/faust_bass_synth/`: the complete DSP and YIN tracker source,
  control integration and tests, plus `regions_EADG.csv` containing
  **timestamps and expected pitches** from an external dry recording;
  the referenced recording is **not in the Git tree**.
- `examples/cpp_loop_freeze/`: a complete loop-freeze effect and tests.
- `examples/faust_bass_mid_drive/`, `faust_parametric_eq/`,
  `faust_stereo_chorus/`, `faust_stereo_slapback/`: additional authored
  example sources.

These are **source files**, not accidentally published build outputs.
Dominik Rogosch confirmed that the bass synth/YIN, loop-freeze, and other
included effect examples should remain publicly available as SDK examples.
They are already accessible in `main` and earlier history. Their inclusion
is an intentional source-publication decision, not a claim that algorithms
or source ownership of independently copyrighted third-party code is waived.

## Preventive controls introduced in PR #3

- Git and Docker ignore lists now include typical build outputs, archives,
  local secrets, credentials and developer databases.
- `scripts/check_release_inventory.py` runs **before** the Docker build in
  the SDK smoke workflow and fails for tracked binaries/archives, credentials,
  unexpected generated directories and oversized source files.
  The legitimate checked-in `adapters/juce/build/CMakeLists.txt` is excluded
  from the generated-build-directory ban.
- This Git-index check is stronger than an ignore list, since Git ignores
  do not affect previously tracked files.

## Limitations

This is a repository inventory and selected content inspection, **not** a
comprehensive historical secret-scan or a full copyright/provenance review.
Unknown secrets could occur inside ordinary text files; no filename-based
test can exclude that. Local developer workspaces and future custom release
assets were not accessible. Check a final GitHub release or exported archive
again if adding non-source assets later. This audit does not imply that
artifacts generated locally under `build/` are publicly distributed.
