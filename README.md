# Wyldtone Plugin SDK — public beta candidate

Build standalone `.wap` WebAssembly audio plugins with Vanilla C++, Faust, or
JUCE. The WAP ABI and manifest are public; the **Wyldtone engine itself is not
included and remains proprietary**.

This is a *release candidate staging copy*, not a signed, legally approved or
fully independently build-verified release. Review `RELEASE-CHECKLIST.md` first.

## Quick start (Linux / Docker)

```sh
# Clone your own JUCE checkout separately only if building JUCE examples.
sudo docker compose build sdk  # one-time image build; omit sudo if Docker is accessible
chmod +x wyldtone              # only necessary if your extraction lost execute bits
./wyldtone test
./wyldtone build examples/gain/plugin.config.json
./wyldtone build examples/faust_gain/plugin.config.json
```

The root-level `wyldtone` launcher invokes Docker Compose for you; it uses
`sudo` only when direct Docker access is unavailable and does not require
changing Docker group membership. Commands executed from an SDK subdirectory
run in that directory in the container. From outside the SDK tree, the working
directory defaults to the SDK root. To call `wyldtone` without `./`, add a
symlink in `~/.local/bin` (see [developer workflow](docs/developer-workflow.md)).

Output: `examples/<example>/build/wyldtone/*.wap`. For JUCE, use the external
`JUCE-upstream` mount described in `docs/juce.md`.


## Quick start (Windows / Docker Desktop)

Install Docker Desktop and enable **Linux containers** (WSL 2 backend is
recommended). In PowerShell from the SDK directory:

```powershell
docker compose build sdk
.\wyldtone.cmd doctor
.\wyldtone.cmd new MyEffect --template cpp
.\wyldtone.cmd build MyEffect
.\wyldtone.cmd validate MyEffect/build/wyldtone/MyEffect.wap
```

The same `.cmd` launcher works in Windows Command Prompt as `wyldtone.cmd`.
It delegates to `wyldtone.ps1`, sets Linux-container UID/GID defaults to 1000,
creates writable cache folders, and preserves the working directory when called
from an SDK subdirectory. On Windows, you do not use `sudo` or `chmod`.
The `.cmd` entrypoint runs the bundled script with a **process-scoped** PowerShell
execution-policy override; it does not change the machine policy. Developers can
review both small launcher scripts before running them.

If you plan to build JUCE examples, keep a separately licensed JUCE checkout
in a sibling directory named `JUCE-upstream`, as required by `compose.yaml`.
Docker Desktop must have access to the Windows drive containing the SDK.

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
- `adapters/cpp`, `adapters/faust`, `adapters/juce`: build frontends
- `examples/`: original plugin examples

## Licensing and security

SDK-owned material is **intended** for Apache-2.0 release; the owner must
confirm provenance and publication rights before publishing. JUCE/AGPL code is
not sublicensed, and generating a JUCE-based WAP can trigger JUCE license
obligations. The marketplace and proprietary host's legal position require
separate review. See [JUCE licensing](docs/juce-licensing.md),
[third-party notices](THIRD-PARTY-NOTICES.md), [security](SECURITY.md), and
[release checklist](RELEASE-CHECKLIST.md).
