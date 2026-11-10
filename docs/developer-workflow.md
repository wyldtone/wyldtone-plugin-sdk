# Developer workflow (SDK CLI)

The public SDK CLI `wyldtone` builds plugins for the open WAP ABI. The Docker
entrypoint forwards the same commands through `docker compose run --rm sdk`.
A root-level `./wyldtone` launcher provides the same commands without typing the
Docker prefix.
Generated projects use only code from the SDK's own sample templates; the JUCE
framework must be obtained and licensed separately.

## Short Docker commands (Linux)

Run once from the SDK root (after extracting or cloning it):

```bash
chmod +x wyldtone
./wyldtone doctor
./wyldtone new MyGain --template cpp
./wyldtone build MyGain
```

The launcher automatically invokes `docker compose run --rm sdk` with the correct
Compose file and project directory. It uses `sudo` only if the calling user
cannot access the Docker daemon directly, so you may still be prompted for your
password. You **do not** need to add your user to the `docker` group (which
confers root-equivalent host privileges). It propagates your numeric UID/GID
for writable Cargo caches and generated files.

To run `wyldtone` without `./`, optionally install a symlink from the SDK root:

```bash
mkdir -p "$HOME/.local/bin"
ln -s "$(pwd -P)/wyldtone" "$HOME/.local/bin/wyldtone"
```

Ensure `~/.local/bin` is on your `PATH`. A symlink can be used from any
working directory: inside an SDK plugin project, `wyldtone build` without
arguments uses that project's `plugin.config.json`; outside the SDK tree,
commands default to `/workspace` (the SDK root), and you should pass a
project path such as `wyldtone build examples/gain`.

This wrapper does not create another image or container service: it runs the
existing Compose `sdk` service. Image rebuilds still use
`sudo docker compose build sdk` (or `docker compose build sdk` if authorized).


## Short Docker commands (Windows)

Docker Desktop must be running in **Linux containers** mode. Install the SDK
on a local drive accessible to Docker Desktop. The SDK toolchain runs **inside**
the Linux build container; no local Rust or Emscripten installation is needed.

In PowerShell from the SDK root:

```powershell
docker compose build sdk
.\wyldtone.cmd doctor
.\wyldtone.cmd new MyGain --template cpp
.\wyldtone.cmd build MyGain
.\wyldtone.cmd inspect MyGain/build/wyldtone/MyGain.wap
```

In CMD use `wyldtone.cmd doctor`, etc. For direct PowerShell execution,
`.\wyldtone.ps1 doctor` also works if your execution policy permits running
local scripts. The `.cmd` form avoids changing system-wide policy by using
a process-local override. Both are thin wrappers around the same Docker
Compose `sdk` service. On Windows, **do not use `sudo`**.

Both launchers preserve the current directory inside the SDK tree. External
working directories fall back to `/workspace`, so provide project-relative
paths such as `examples/gain` when calling from elsewhere. If you copy the
SDK or use spaces in its Windows path, keep the `.cmd`, `.ps1`, and
`compose.yaml` in the same root folder. The default Linux UID/GID in
Docker Desktop is 1000:1000; set `HOST_UID` and `HOST_GID` in your shell
only if you intentionally need different IDs.

The JUCE mount is separate from the SDK, and `../JUCE-upstream` in Compose
refers to a **sibling directory on the Windows host**. JUCE itself is not
shipped with the SDK.

## Prerequisites

Run `docker compose build sdk` once for the documented Linux build environment.
If using the native CLI, install Rust, Emscripten (`em++`, `emcmake`), CMake,
and Ninja. Faust is needed only for Faust builds, and an external JUCE checkout
is needed only for JUCE builds. `wyldtone doctor` prints detected tools and
reports missing essential build tools. An unavailable optional framework is
reported as a warning.

## Create and build

```sh
# Run in the SDK workspace; new refuses to overwrite any existing directory.
./wyldtone new MyGain --template cpp
./wyldtone new MyFaust --template faust
./wyldtone new MyJuce --template juce
# Optional: --manufacturer 'Your Company'

./wyldtone build MyGain
# An explicit config path still works:
./wyldtone build MyFaust/plugin.config.json --dsp-profile quick
```

A native CLI running in a plugin's own directory also accepts `wyldtone build`
without a config path. The short launcher preserves the invoking directory when it is inside the SDK
workspace. The raw `docker compose run` command defaults to `/workspace`, so
pass the plugin directory or set the working directory explicitly.

`new` generates a `plugin.config.json`, example source, `.gitignore`, and
README. The generated ID uses the placeholder namespace `org.example` and must
be changed before uploading to any marketplace. Rename the producer field and
review the default version, category, declared audio buses, and license.

### Generate plugin.config.json for an existing JUCE project

Use the existing sources instead of copying the sample config:

```bash
./wyldtone init-juce projects/obsidian_b7000 \
  --id com.leighpierce.obsidianb7000 \
  --name "Obsidian B7000" \
  --description "Circuit-modelled bass preamp and overdrive"
```

The generator reads source files without executing the project's CMake scripts.
It detects a single AudioProcessor class in a root/src header, processor header path,
CMake project VERSION and COMPANY_NAME, simple mono/stereo audio buses,
juce_dsp headers and known header-only dependency include roots.
It creates plugin.config.json in the project root and refuses to overwrite it.

Discovery is **best-effort**, not a general C++ or CMake parser. It warns if audio
buses could not be resolved (and omits them) or if version/manufacturer require
fallback values. If more than one processor class is found, it fails instead of
guessing. Review all generated fields, including audio buses, manufacturer,
version, modules, includes and licensing, before distributing a WAP.
Complex third-party CMake builds may need manual configuration or SDK support.

For external projects, provide the container-visible path mounted through
compose.override.yaml (see the external projects section). Then run:

```bash
./wyldtone build projects/obsidian_b7000/plugin.config.json
```

### JUCE notes

The JUCE template uses an original minimal processor written for this SDK,
not the JUCE framework sources or a vendored JUCE checkout. Configure `JUCE_DIR`
as described in [JUCE licensing](juce-licensing.md). The compiler combines
against the user's JUCE checkout; the resulting binary may be governed by the
JUCE licensing terms. An Apache-2.0 SDK does **not** grant commercial JUCE
rights or override obligations of AGPLv3.

## Inspect and validate

```sh
./wyldtone inspect MyGain/build/wyldtone/MyGain.wap
./wyldtone inspect MyGain/build/wyldtone/plugin.wasm
./wyldtone validate MyGain/build/wyldtone/MyGain.wap
```

`inspect` prints metadata, bus layout and the parameter list. For `.wap`, it
also checks the manifest against the actual WASM metadata. `validate` accepts
WAP packages and checks:

- Manifest version, required plugin metadata, and duplicate parameter IDs.
- WASM module loadability, exports/parameter ABI, and declared bus layout.
- Agreement between packaged parameter metadata and live WASM metadata.
- Presence and basic constraints of embedded icon artwork, if present.

Validation is **not** a malware audit, a licensing assessment, proof of
real-time safety or a proof that every effect parameter is audible. Uploaded
untrusted plugins should be validated and benchmarked in a separately isolated
service with execution-time and resource limits. `test-dsp` covers functional
parameter changes; see [DSP tests](dsp-tests.md). The standalone
[offline WAP reference host](reference-host.md) renders WAV files without an
audio device or proprietary Wyldtone engine.

## Troubleshooting

- `Required build tool not found`: check `doctor` and your Docker image.
- `Plugin configuration not found`: pass the project directory or its
  `plugin.config.json`; relative paths are relative to `/workspace` in Docker.
- `JUCE checkout ... unavailable`: provide an external checkout and set
  `JUCE_DIR` (Docker uses `/opt/juce`).
- `WAP parameters differ`: rebuild the WAP from matching `.wasm` and
  `manifest.json`, then run `validate` again.
- `New project already exists`: choose another directory; `new` never
  overwrites an existing project.

## Distribution and licensing

Before distributing plugins built with the SDK, review [third-party notices](../THIRD-PARTY-NOTICES.md),
[JUCE licensing](juce-licensing.md), and [security guidance](../SECURITY.md).
The proprietary Wyldtone engine is not included in this public repository.
No JUCE sources are embedded in this template generator.
