# SDK beta — dependency license audit (2026-11-06)

**Status:** initial dependency-license inventory, **not legal clearance**.

## Evidence and method

- Current `main` has five local Rust workspace crates and 286 package
  entries in `Cargo.lock`.
- An isolated GitHub Actions run of `cargo metadata --format-version 1 --locked`
  found **281 external Rust package entries**. The artifact
  `rust-third-party-license-inventory` in
  [license audit run #1](https://github.com/wyldtone/wyldtone-plugin-sdk/actions/runs/42191597843)
  has every package's reported SPDX license and a review list.
- **276/281** external entries were classified as declared permissive under the
  script's conservative whitelist. **5/281** were flagged for manual review;
  those flags do not, by themselves, indicate a license conflict.
- This is a *resolved metadata inventory*, not an SBOM of the final distributed
  binary or exhaustive bundled C/C++ sources. Verify full license files,
  copyrights, exception text, transitive native code and distribution notices.

## Five review items

| Cargo.lock package | SPDX declaration | Initial assessment |
|---|---|---|
| `ittapi 0.4.0` | `GPL-2.0-only OR BSD-3-Clause` | BSD-3-Clause alternative available according to its package metadata; inspect included Intel ITT source file notices, not just crate metadata |
| `ittapi-sys 0.4.0` | `GPL-2.0-only OR BSD-3-Clause` | Same; check code and license files actually included |
| `r-efi 5.3.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | MIT/Apache-2.0 are permissive alternatives |
| `r-efi 6.0.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | MIT/Apache-2.0 are permissive alternatives |
| `libbz2-rs-sys 0.2.6` | `bzip2-1.0.6` | SPDX-defined permissive-style license; retain original copyright, disclaimers and mark modifications |

**Preliminary conclusion:** No outright incompatibility was identified from
the 281 external Rust packages' declared SPDX expressions, assuming the
permissive options are valid for the actual distributed sources. This is
not proof that all copyright, third-party notice and license-file obligations
have been satisfied.

## Build tools and external frameworks

| Component | Use | Distribution implication |
|---|---|---|
| Wasmtime/Cranelift/WASI | Rust runtime dependency | Generally Apache-2.0 with LLVM exception; retain required notices and verify exact resolved crates |
| Emscripten/LLVM | Developer toolchain in SDK Docker image | Emscripten is MIT/NCSA; LLVM uses Apache-2.0 with LLVM exception. Build tools' licenses do not automatically relicense generated WAPs. Docker image distribution requires separate layer/package inventory |
| Faust compiler | Installed in SDK Docker build container | Compiler is copyleft (Faust FAQ describes LGPL); its license does not automatically apply to emitted code. Check the specific imported Faust libraries and architecture files used in generated plugins |
| JUCE | Externally provided optional build framework | JUCE modules are AGPLv3 or commercial licensed; this cannot be treated as Apache-2.0 material. JUCE-containing WAPs have separate distribution obligations |
| Debian packages, Rust toolchain, shell/CLI tools, image layers | SDK Docker image | Not covered by `Cargo.lock`; need a container/package-level inventory before **redistributing** the image |
| Optional third-party JUCE modules / developer supplied dependencies | Developer-provided source | Not part of this repository's dependency inventory; developers must license them separately |

## Repository-specific items still blocking public release

1. Confirm rights/provenance for original and engine-derived SDK source and
   contributor material before licensing those sources under Apache-2.0.
2. Review `ColourCompat.cpp`, `HeadlessEditorCompat.h`, and JUCE VFS-generated
   compatibility source against the upstream JUCE copyright terms.
3. Replace the tentative `NOTICE` owner declaration and preserve third-party
   notices actually required by selected dependency licenses.
4. Inspect license files and bundled native code of flagged Rust packages,
   and produce a license/notice inventory for the *distribution artifact*
   (especially if distributing the Docker toolchain image).
5. Confirm the Faust standard library imports and architecture code compiled
   into the sample WAP, and the JUCE edition/licensing assumptions for the
   documented supported JUCE checkout.

## Sources

- SPDX bzip2 license text: https://spdx.org/licenses/bzip2-1.0.6.html
- r-efi declared alternatives: https://github.com/r-efi/r-efi
- ITT API declared alternatives: https://github.com/intel/ittapi
- Emscripten license: https://emscripten.org/docs/introducing_emscripten/emscripten_license.html
- Faust compiler vs generated code: https://faustdoc.grame.fr/manual/faq/
- JUCE upstream licensing: https://github.com/juce-framework/JUCE/blob/master/LICENSE.md
- Wasmtime upstream: https://github.com/bytecodealliance/wasmtime

**Checklist recommendation:** Do **not** yet check the full
"Generate dependency SBOM and third-party licence report (Rust, Docker,
Emscripten, Faust, JUCE)" item. Its **Rust SPDX inventory** is completed;
full distribution SBOM, notices, external toolchain and source provenance
remain unresolved.
