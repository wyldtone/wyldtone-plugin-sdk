# Public WAP format — interoperability contract (current SDK beta)

WAP is a ZIP container containing **`plugin.wasm`** and **`manifest.json`**;
optional artwork may be `icon.png` or `icon.svg`. `format_version` currently 1.
The schema is defined in `crates/wyldtone-plugin-api/src/manifest.rs`.
The plain C ABI is defined in `abi/include/wasm_plugin.h`.

The WASM exports and metadata are deliberately available without installing the
proprietary Wyldtone engine. Hosts can implement them using Wasmtime or another
conforming WebAssembly runtime; the SDK contains `wap-host` as an independent,
non-real-time rendering reference.

## Core ABI (overview)

- `memory`, `plugin_init(sample_rate, channels, block_frames)`,
  `plugin_buffer_ptr()`, `plugin_process(pointer, float_count)`.
- `plugin_create` and `plugin_reset` may be available. Parameters: count, id,
  name, normalized get/set, bounds, default, flags and optional choice labels.
- Static bus layout is optionally published via the four bus-count/channel
  functions; newer examples use `plugin_process_buses(pointer, frames)` with
  input-interleaved floats followed by output-interleaved floats.
- Optional state ABI exports describe capture/restore. See the C header for
  exact function signatures and lifecycle.
- Audio data consists of **little-endian IEEE float32** samples. The reference
  host currently handles only one symmetric mono or stereo bus. It does not
  implement real-time scheduling, sidechains, latency compensation or a graph.

The SDK runtime is currently the reference for compatibility. The public API
must be versioned before declaring a long-term binary-compatibility guarantee.
Extensions should add optional exports or negotiate explicit ABI revisions.

## Future signing and licensing (proposal only)

Publisher integrity signatures and independently signed license grants are
being designed as **optional**, backwards-compatible extensions. The intended
license validation runs inside the plugin, including with third-party hosts;
no license ABI or DRM enforcement is implemented in this SDK yet. See
[licensing and signatures](licensing-and-signatures.md) for the provisional
schema, trust boundaries and open design decisions.

### Optional license transfer ABI (proposal v1)

`abi/include/wasm_plugin.h` now declares four optional `plugin_license_*`
exports and a `WASM_PLUGIN_EVENT_LICENSE_REQUESTED` event flag. A compatible
host can deliver an embedded, separately signed license from the WAP when the
plugin requests it. Existing plugins and hosts do not require this extension.
The protocol is specified in [licensing and signatures](licensing-and-signatures.md),
but no host/packager runtime support is implemented or tested yet.
