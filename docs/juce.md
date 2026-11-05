# JUCE plugin builds (experimental compatibility)

The SDK does not ship JUCE. Obtain an external licensed JUCE checkout yourself;
the tested development baseline is commit `7aae7d8e8`.

On Linux:

```sh
git clone https://github.com/juce-framework/JUCE.git ../JUCE-upstream
git -C ../JUCE-upstream checkout 7aae7d8e8
export JUCE_DIR="$(realpath ../JUCE-upstream)"
export WYLDTONE_JUCE_MODE=upstream
cargo run --release -p wyldtone-cli -- build examples/juce_gain/plugin.config.json
```

The `sdk` Docker Compose service mounts `../JUCE-upstream` read-only at
`/opt/juce`; the original checkout is not modified. GUI-only editor methods
are staged into temporary headless source copies. The SDK's CMake VFS overlay
may create locally modified JUCE implementation files; those copies retain
JUCE licensing and must not be published as Apache source.

This backend is currently compatible with a known JUCE commit and a subset of
AudioProcessor patterns, **not every JUCE project or version**. Additional
modules/headers may require changes. No GUI, plugin-initiated threading or
JUCE framework binaries are distributed with the SDK.

See `docs/juce-licensing.md` for licensing distinctions and `THIRD-PARTY-NOTICES.md`.
