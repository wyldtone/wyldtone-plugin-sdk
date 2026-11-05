# Faust backend

This is the first executable Faust backend, replacing the old placeholder.

* `wyldtone_arch.cpp` is a minimal Faust architecture used with `faust -i`;
  it embeds Faust's standard DSP and UI interfaces alongside the generated class.
* `wyldtone_bridge.cpp` maps Faust DSP/UI to Wyldtone's C/WASM ABI.
* `crates/wyldtone-cli/src/faust.rs` calls the compiler and invokes the
  existing C++ WASM builder.

Run the example with:

```bash
sudo docker compose build sdk
sudo docker compose run --rm sdk build examples/faust_gain/plugin.config.json
```

See [docs/faust.md](../../docs/faust.md) for the limitations and developer guide.
