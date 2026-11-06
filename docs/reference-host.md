# Offline WAP reference host

`wap-host` serves **two purposes**: a small offline test tool for plugins
built with the SDK, **and a reference implementation for third-party host
developers** who want to load and run WAP plugins in their own applications.
It demonstrates WAP package loading, manifest inspection, WASM instantiation,
parameter exchange and audio processing through the public ABI.

The implementation lives in [`crates/wyldtone-wap-host`](../crates/wyldtone-wap-host)
and uses the shared [public WASM runtime adapter](../crates/wyldtone-plugin-runtime)
and [WAP packager](../crates/wyldtone-plugin-packager). Developers of other
hosts can study these components as a concrete example, without depending on
the proprietary Wyldtone engine. An independent host may use Wasmtime or
another compatible WebAssembly runtime.

This is deliberately a **minimal, non-real-time example**, not a full host
framework or a drop-in implementation of every WAP capability. It currently
supports an offline mono/stereo render to WAV, and **does not** include the
proprietary Wyldtone engine, graph, monitoring, device drivers, or real-time
processing. The proposed optional license delivery ABI is documented, but
**not yet implemented in this reference host**. When licensing is implemented
and validated in Wyldtone, the reference host can be updated to demonstrate it.
See [WAP format](wap-format.md) and [licensing proposal](licensing-and-signatures.md).

## Build

```sh
cargo build --release --locked -p wyldtone-wap-host
```

With the SDK Docker image (rebuild once after SDK update):

```sh
docker compose run --rm sdk host inspect examples/gain/build/wyldtone/SDK-Gain-Example.wap
```

Substitute the actual generated package file (check `examples/gain/build/wyldtone/`).

## Process a WAV file

```sh
cargo run --release --locked -p wyldtone-wap-host -- render \
  my-effect.wap --wav input.wav --out processed.wav --block 256
```

Generate and process a test tone instead:

```sh
cargo run --release --locked -p wyldtone-wap-host -- render \
  my-effect.wap --test-tone 110 --duration 2 --rate 48000 --out test-output.wav
```

Use `--param gain=0.5` to set a normalized plugin parameter; use `inspect` to
see the parameter IDs. For delays/reverbs you can add `--tail 2.0` to append two
seconds of silence for rendering the plugin tail.

The WAV input must be uncompressed PCM 16/24/32-bit or IEEE float32, mono/stereo;
the plugin must declare the **same** channel count. The output is float32 WAV
at the input sample rate, with no resampling or volume normalization.
Unsupported layouts (e.g., sidechains, generators, surround buses) produce
an explicit error instead of silently changing channels.

## Limitations and security

This is **offline rendering**, not a DAW and not a real-time host. It neither
schedules plugins nor guarantees latency alignment; it does not correct for
plugin-reported latency. Unsupported WAV variants (including WAVE_FORMAT_EXTENSIBLE)
need conversion first. Its fixed memory/block limits and simple CLI are intended
for demos and trusted local plugins, not hostile public upload processing.

A WASM sandbox alone does not bound execution time or resources. For untrusted
packages use an isolated, resource-limited external process with a runtime fuel
or epoch policy and a stricter package parser; do not run this CLI as a privileged
Marketplace processing service.
