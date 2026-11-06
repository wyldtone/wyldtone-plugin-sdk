# JUCE plugin builds (experimental compatibility)

The SDK does not ship JUCE. Obtain an external licensed JUCE checkout yourself;
the tested development baseline is commit `7aae7d8e8`.

### Docker-based build (recommended)

From the SDK root on Linux, supply an external JUCE checkout at the
**sibling** path expected by `compose.yaml`. The public CI validates the
specific upstream commit shown below:

```bash
git clone https://github.com/juce-framework/JUCE.git ../JUCE-upstream
git -C ../JUCE-upstream checkout 7aae7d8e8deb8413bb01633d2795ef9974a181c5
docker compose build sdk
./wyldtone build examples/juce_gain/plugin.config.json
./wyldtone inspect examples/juce_gain/build/wyldtone/JUCE-Gain-Example.wap
```

On Windows with Docker Desktop, use the same sibling JUCE checkout and
`docker compose build sdk`, then replace `./wyldtone` with
`.\wyldtone.cmd` in the last two commands.

The container already sets `JUCE_DIR=/opt/juce` and
`WYLDTONE_JUCE_MODE=upstream`. Setting `JUCE_DIR` in the host shell is
not necessary for Docker builds. For native (non-Docker) builds, configure
those variables yourself and install the full Rust/CMake/Emscripten toolchain.
JUCE is licensed separately; see [JUCE licensing](juce-licensing.md).

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
