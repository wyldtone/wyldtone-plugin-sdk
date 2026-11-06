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


## JUCE source-to-source comparison (baseline `7aae7d8e8`)

Compared the SDK sources on `main` against the public
[`juce-framework/JUCE` revision `7aae7d8e8`](https://github.com/juce-framework/JUCE/tree/7aae7d8e8).

| SDK file / technique | JUCE comparison | Release significance |
|---|---|---|
| `adapters/juce/headless/ColourCompat.cpp` | **The `Colour(uint32)` constructor body is functionally and textually nearly identical** to JUCE's `modules/juce_graphics/colour/juce_Colour.cpp`: same channel extraction expressions, same component order and `argb` initialization. SDK uses different argument identifiers. | **Remediated in this draft PR:** the SDK shim has been rewritten to call the published `PixelARGB::setARGB()` API rather than reproduce JUCE's `Colour.cpp` member-initializer implementation. The published 32-bit ARGB byte layout necessarily remains the same. Automated build/render regression test added for the supported external JUCE revision; success must be checked before clearance. |
| `adapters/juce/bridge/HeadlessEditorCompat.h` | JUCE declares real `AudioProcessorEditor` (derived from `Component`) and `GenericAudioProcessorEditor` in its `juce_audio_processors/processors` headers. SDK substitutes tiny no-op classes with compatible names and a trivial constructor, **not copies of the original class bodies or functionality**. | No substantial copied implementation identified; this is a compatibility shim, but its provenance remains for the maintainer to attest. Be cautious: name/class compatibility does not make the actual JUCE graphics API supported. |
| `adapters/juce/bridge/WasmPlatformCompat.cpp` | Implements three JUCE-declared methods with no-op/`false` WASM stubs. JUCE has OS-specific thread/message dispatch implementations; these tiny WASM bodies are distinct from upstream platform implementations. | No copied JUCE implementation identified. |
| `adapters/juce/bridge/HeadlessAPVTS.cpp` | Includes JUCE's `juce_AudioProcessorValueTreeState.cpp` by path in **the developer's own JUCE checkout**; no JUCE implementation text is embedded in the SDK copy. | Source-only SDK does not redistribute that file. The developer's compiled WAP may incorporate JUCE, under their JUCE license obligations. |
| `adapters/juce/build/CMakeLists.txt` VFS transform | Reads four upstream files from the developer's checkout, adds small guarded WASM conditional changes in temporary local copies, and overlays compilation paths. Baseline source anchors are present in the upstream revision. | The Git-tracked SDK contains patch **instructions**, not copied JUCE files. Generated derivatives retain JUCE license rights and must not be published as Apache-only SDK files. A bundled prebuilt JUCE WAP would need separate review. |
| `examples/juce_gain/PluginProcessor.cpp/.h` | Small gain processor using the JUCE public API; no substantial upstream source code identified in this spot check. | Compatible with a source-only Apache release **conditional on original authorship and rights**. Do not bundle the built JUCE WAP. |

**Key distinction:** This is a textual and architectural comparison of the
identified compatibility paths, **not a legal originality determination**.
API compatibility alone is not copying an implementation, but minimal code
similarity (especially the `Colour` constructor) warrants documenting or
removing the overlap before asserting exclusive Apache-2.0 licensing.

### Personal release owner and future corporate ownership

The releasing maintainer states that **they will personally publish the SDK**
for the initial beta; no GmbH currently needs to be named as publisher.
The `NOTICE` must carry the legal name of the **actual copyright holder(s)**,
based on existing rights and contributor agreements; GitHub-account identity
alone does not establish legal title. If a GmbH is later formed, a separate
written rights transfer or license, plus an updated notice for subsequent
versions, may be appropriate. Changing a notice does **not** by itself transfer
copyright, and historical Apache-2.0 grants already made cannot simply be
withdrawn retroactively.

## Repository-specific items still blocking public release

1. Confirm rights/provenance for original and engine-derived SDK source and
   contributor material before licensing those sources under Apache-2.0.
2. Source comparison of `ColourCompat.cpp`, `HeadlessEditorCompat.h`, and VFS-generated
   compatibility source against pinned JUCE was performed; original `Colour` shim
   overlap was replaced with public-API-based shim. Confirm green JUCE build/render
   CI and maintainer-originated source rights before closing this item.
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
