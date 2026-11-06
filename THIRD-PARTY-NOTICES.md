# Third-party notices and distribution boundaries

This repository does **not** vendor JUCE, the Emscripten SDK, Faust, Rust,
Wasmtime or the original third-party example projects.

| Component | Used by | Distribution / license consideration |
|---|---|---|
| JUCE | Optional JUCE build backend | Obtained separately by the developer. JUCE framework is offered under AGPLv3 or commercial JUCE terms; see [JUCE licensing](docs/juce-licensing.md). This repository is not a JUCE sublicense. |
| Emscripten / LLVM | Local WASM builds | Separately installed compiler toolchain; comply with their respective notices when redistributing containers. |
| Faust compiler / architecture headers | Optional Faust backend | Separately installed. The example uses original `gain.dsp` without standard-library imports. `faust/dsp/dsp.h`, `faust/gui/UI.h`, `faust/gui/meta.h` identify GRAME as copyright holder and use LGPL-2.1-or-later with a special exception for unmodified sections embedded into larger works. Generated code and WAPs stay local and are not part of this source-only release; see [license audit](docs/license-audit-beta.md). |
| Rust crates / Wasmtime / WASI | CLI and reference host | Cargo.lock lists the dependency versions, each governed by its own upstream license. Generate an SBOM/licence inventory for release. |
| Docker base images and Debian packages | Optional developer Docker image | Image layers and system packages have their own upstream licenses. The Docker image is a developer convenience, not a blanket Apache-2.0 relicensing. |

## JUCE compatibility implementation

- `adapters/juce/bridge/WasmExport.cpp`: WAP export adapter written for the SDK; it invokes JUCE API surface.
- `adapters/juce/bridge/HeadlessAPVTS.cpp`: thin C++ wrapper that includes JUCE's APVTS implementation **from the developer's installed checkout**. No copied JUCE APVTS `.cpp` is distributed in this repository.
- `adapters/juce/bridge/WasmPlatformCompat.cpp`: small independently authored WASM host stubs; compared with the public upstream JUCE implementation as recorded in [the beta license audit](docs/license-audit-beta.md).
- `adapters/juce/bridge/HeadlessEditorCompat.h` and `adapters/juce/headless/ColourCompat.cpp`: narrow SDK-owned compatibility implementations of JUCE-facing class interfaces. The previously close-matching `Colour` constructor was rewritten to use JUCE's public `PixelARGB` setter; see [beta license audit](docs/license-audit-beta.md). Compatibility with future JUCE revisions must still be retested.
- `adapters/juce/build/CMakeLists.txt`: patches a few externally sourced JUCE files into *temporary build copies* using an LLVM VFS overlay. Those derivative copies are JUCE material and remain local; they are not Apache-licensed SDK source.

Historic JUCE source patches, private test fixtures, and generated JUCE/VFS outputs
were removed from the public distribution staging tree.

This is a technical inventory, **not a legal opinion** or proof that every source
file is cleared for public relicensing. Ownership/provenance, Apache NOTICE
obligations, licenses of transitive dependencies and generated images require a
final review by the releasing maintainer.
