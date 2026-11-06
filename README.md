# Wyldtone Plugin SDK — public beta

Build standalone `.wap` WebAssembly audio plugins with Vanilla C++, Faust, or
JUCE. The WAP ABI and manifest are public; the **Wyldtone engine itself is not
included and remains proprietary**.

**Beta version:** `0.2.0-beta.1` (source-only). This repository provides the SDK source;
there are no prebuilt WAPs, Docker images or JUCE framework files in the release.
The beta is not a security certification or an independent legal assessment.
See [third-party notices](THIRD-PARTY-NOTICES.md) and [security guidance](SECURITY.md)
for distribution boundaries and known limitations.

## Your first plugin (Linux / Docker)

Requirements: **Git, Docker Engine with Docker Compose v2**, and a Linux
x86-64 (amd64) machine with enough free disk space for the toolchain build.
The first Docker build downloads compiler and Rust dependencies and may take
some time. No local Rust, C++ compiler or Faust installation is needed.

Run these commands from a terminal:

```bash
git clone https://github.com/wyldtone/wyldtone-plugin-sdk.git
cd wyldtone-plugin-sdk
docker compose build sdk
./wyldtone doctor
./wyldtone new MyGain --template cpp
./wyldtone build MyGain
./wyldtone validate MyGain/build/wyldtone/MyGain.wap
./wyldtone host render MyGain/build/wyldtone/MyGain.wap \
  --test-tone 110 --duration 2 --rate 48000 --out MyGain/build/wyldtone/demo.wav
```

The output is `MyGain/build/wyldtone/MyGain.wap`, with a rendered
`demo.wav` to play in any audio player. Open `MyGain/plugin.cpp`, modify
the effect, and run `./wyldtone build MyGain` again. `new` creates a
starter project; it refuses to overwrite an existing directory.
Replace the generated `org.example.*` ID and manufacturer before
publishing your own plugin.

If Docker needs root access, use `sudo docker compose build sdk` for the
image build. The `./wyldtone` launcher uses `sudo` for container
commands only when direct Docker access is unavailable. Avoid adding
your account to the `docker` group solely for this SDK; Docker daemon
access grants elevated host privileges.

The `compose.yaml` mounts a sibling `../JUCE-upstream` directory.
It may be empty for C++ and Faust work; JUCE itself is **not required**
for this quickstart.

### Windows (Docker Desktop)

Requirements: Git, Docker Desktop with **Linux containers** (WSL 2 backend),
and a local folder accessible to Docker Desktop. Run in PowerShell:

```powershell
git clone https://github.com/wyldtone/wyldtone-plugin-sdk.git
cd wyldtone-plugin-sdk
docker compose build sdk
.\wyldtone.cmd doctor
.\wyldtone.cmd new MyGain --template cpp
.\wyldtone.cmd build MyGain
.\wyldtone.cmd validate MyGain/build/wyldtone/MyGain.wap
.\wyldtone.cmd host render MyGain/build/wyldtone/MyGain.wap --test-tone 110 --duration 2 --rate 48000 --out MyGain/build/wyldtone/demo.wav
```

The Windows launcher calls PowerShell with a process-scoped execution-policy
override, without changing the system policy. Output files remain in the
local SDK folder. For optional JUCE builds, place a licensed checkout at
`../JUCE-upstream` (sibling to the SDK directory); see [JUCE guide](docs/juce.md).

### Supported and unverified environments

The SDK is continuously tested on **GitHub-hosted Linux x86-64** runners.
The Windows launcher and Docker Desktop instructions are provided, but
there is no separate Windows CI runner. macOS, including Apple Silicon
(arm64), has **not been validated by CI**; platform-specific build issues
may occur. The JUCE backend is experimental and tested against a specific
external JUCE revision.

## Where can I run a WAP plugin?

The **public, offline WAP reference host** included in this repository
can inspect WAPs and process mono/stereo WAV files or generated test tones
without a sound card. It is a development and interoperability tool, not a
real-time DAW, live instrument processor or secure service for unknown plugins.

The **actual Wyldtone audio host/engine is proprietary and currently in
private testing. It is not publicly available yet.** When a public
host beta becomes available, installation and plugin-loading instructions
can be linked here. You do **not** need access to that private host to build
and test plugins with this SDK.

See [developer workflow](docs/developer-workflow.md),
[offline reference host](docs/reference-host.md), and [WAP format](docs/wap-format.md).

## Create, inspect, and validate plugins

The SDK CLI includes convenience commands. The examples below run through
Docker, with generated files written into your mounted SDK workspace:

```sh
./wyldtone doctor
./wyldtone new MyEffect --template cpp
./wyldtone new MySynth --template faust
./wyldtone new MyJuceEffect --template juce

./wyldtone build MyEffect
./wyldtone inspect MyEffect/build/wyldtone/MyEffect.wap
./wyldtone validate MyEffect/build/wyldtone/MyEffect.wap
```

New projects contain their own `plugin.config.json`, source files and
`.gitignore`. Change the generated `org.example.*` plugin ID and placeholder
manufacturer before publishing. JUCE is never copied into a generated project;
its framework license remains separate.

Run `wyldtone build` directly inside a project directory if using the installed
CLI without Docker. `validate` checks WAP structure, manifest/actual WASM
parameter metadata, exports and artwork; **it does not certify safety or
real-time behavior**. See [developer workflow](docs/developer-workflow.md).

## WAV in → WAP plugin → WAV out (no real-time audio)

```sh
# First build the example to obtain its WAP package.
./wyldtone build examples/gain/plugin.config.json
# Supply the exact WAP file created by the build.
./wyldtone host render \
  examples/gain/build/wyldtone/SDK-Gain-Example.wap \
  --wav input.wav --out output.wav

# Or generate a 110 Hz sine test signal:
./wyldtone host render \
  examples/gain/build/wyldtone/SDK-Gain-Example.wap \
  --test-tone 110 --duration 2 --rate 48000 --out test-output.wav
```

The host supports a single matching mono or stereo bus and writes float32 WAV.
Beyond testing finished plugins, it is an **open-source implementation example
for developers building their own WAP-compatible hosts**: it demonstrates how
to load a WAP, interpret its manifest, instantiate the WASM module, communicate
through the public ABI, and process audio. It is deliberately minimal and
**not** the proprietary Wyldtone engine or a complete DAW/real-time host.
See [reference host](docs/reference-host.md) and [WAP format](docs/wap-format.md).

## Contents

- `abi/include/wasm_plugin.h`: WAP audio/parameter/state ABI
- `crates/wyldtone-plugin-api`: manifest and configuration schemas
- `crates/wyldtone-plugin-runtime`: public WASM runtime adapter
- `crates/wyldtone-plugin-packager`: WAP packaging
- `crates/wyldtone-cli`: build, DSP tests and benchmarks
- `crates/wyldtone-wap-host`: **minimal offline WAP reference host**
- `adapters/faust`, `adapters/juce`: Faust/JUCE build frontends; Vanilla C++ builds are handled by `crates/wyldtone-cli/src/cpp.rs`
- `examples/`: original plugin examples

## Licensing and security

Original SDK source code for which Dominik Rogosch holds the required rights
is published under Apache-2.0; independently copyrighted third-party material
retains its own license and notice requirements. JUCE/AGPL code is
not sublicensed, and generating a JUCE-based WAP can trigger JUCE license
obligations. The marketplace and proprietary host's legal position require
separate review. See [JUCE licensing](docs/juce-licensing.md),
[third-party notices](THIRD-PARTY-NOTICES.md) and [security](SECURITY.md).
