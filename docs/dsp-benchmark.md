# Wyldtone SDK: DSP Benchmark and Marketplace integration

## Purpose

`wyldtone benchmark` measures how long **the WASM processing call** takes on a
particular CPU with a particular Wasmtime/runtime/toolchain. It does not measure
JUCE/NAM worker IPC, the full Wyldtone audio graph, plugin creation, or buffer
copying. It is **not** a real-time guarantee or a universal CPU percentage.

Benchmark results must only be compared if the protocol, software versions,
reference host, operating conditions and parameter scenario match.

## SDK CLI

```bash
sudo docker compose run --rm sdk benchmark \
  examples/gain/build/wyldtone/plugin.wasm \
  --reference-id wyserver-v1 --scenario v2_oversampling_4x --samples 300 \
  --param revision=1 --param oversampling=0.667
```

The optional `--scenario` is a stable, human-readable scenario identifier.
Run a default and an opt-in higher-load scenario where supported.

`--param` values are **normalized 0..1**, not raw device values or the names
of choice entries. Verify how each plugin maps those values, and document the
scenario. For plugins with Choice parameters, *do not assume* `oversampling=0.667` means exactly 4x
without checking the actual exported choice list; treat it only as an example.
Unknown parameter IDs, duplicate overrides and values outside 0..1 are errors.

Outputs `dsp-benchmark-report.json` next to `plugin.wasm` by default. Override
with `--report PATH`. Each sample runs one block after 48 untimed warm-up
blocks, and 32..2000 blocks per block size can be requested. The initial
protocol supports 48 kHz, 128/256/512 frames, one matching mono/stereo input
and output bus, and fixed deterministic audio. Sidechains and other multi-bus
layouts need separate fixtures and are rejected rather than mismeasured.

### Interpret the report

- `median_ms`: typical call duration on this host.
- `p95_ms`: 95th percentile duration; useful for seeing occasional spikes.
- `audio_budget_ms`: e.g. 512/48000 seconds = 10.667 ms.
- `p95_budget_percent`: 100 * p95 / audio budget. **Not** general CPU usage.
- `max_ms`: worst measurement in this short sample; often load-sensitive.
- `scenario`: label for the validated parameter configuration; do not silently
  replace it without rerunning the measurement.
- `reference_id`: operator-controlled identifier for hardware/software
  baseline; increment whenever CPU, virtualization, runtime or CPU affinity
  changes materially.

A 2 ms call at 512 frames/48 kHz consumes about 18.75% of the available
block window **in this specific measurement**, not 18.75% of all server CPUs.
Do not advertise this as an absolute score for customers' devices.

### Marketplace ingestion (design contract, NOT yet wired to Spree)

1. Upload enters **pending-validation**. Enforce upload size limits, ZIP/WAP
   validation, required exports and manifest constraints. Record SHA-256.
2. Enqueue a **single** benchmark job. Never execute uploaded code in the
   web/Spree container or give that process the Docker socket.
3. A dedicated worker receives a *copy* of the validated `plugin.wasm` in a
   per-upload, unprivileged staging directory. Do not mount shop DB/secrets.
4. Execute the standalone benchmark image with no network, non-root user,
   read-only root filesystem, low CPU/memory/PID limits and an **external
   wall-clock timeout**. The worker must forcibly stop stuck containers.
5. Record the JSON result, input SHA-256, benchmark image digest, reference
   host ID, timestamp, software/runtime version, parameter scenario and job
   status in the marketplace database. Successful benchmark is separate from
   malware/security validation and functional DSP tests.
6. Publish measurements only for completed, reproducible runs. Queue retries
   for noisy/aborted runs and label unsupported layouts as "not measured".

For a *manual prototype*, build the isolated image on `wyserver`:

```bash
docker compose build sdk
docker build -f docker/Dockerfile.benchmark -t wyldtone/plugin-benchmark:local .
```

Then use `marketplace/benchmark-upload-example.sh` from a trusted job worker,
not an HTTP request thread. The script expects a private directory such as
`/srv/benchmark-jobs/123/plugin.wasm`, writable by UID 10001 and with the
uploaded file already validated. It writes `benchmark.json` and
`plugin.sha256`. The Spree upload integration is **not** included in this patch.

### Reference hardware and load

`wyserver` has 2 CPU cores and also runs the Spree storefront/database. A
container limited to 1 CPU can still be delayed by competing host processes.
Run one job at a time, ideally during low shop activity; log host load and
repeat benchmarks on different occasions. For authoritative Marketplace
comparisons a separate (even small) reference worker host is preferable.
Do not treat benchmark results across different SDK image digests, CPU types
or compiler settings as directly comparable. Persist an explicit protocol
version, and recalibrate/rebenchmark after host changes.

### Safety and limitations

A WASM runtime limits the guest's capabilities, but CPU loops and memory DoS
still need process/container time and resource limits. The SDK runtime permits
no WASI preopened directories, but the **benchmark process itself** needs a
writable directory for its Wasmtime cache and JSON output. The example
assumes the upload-side service has already performed security validation;
it is not a substitute for a dedicated secure execution review. Do not run
unknown plugins with a privileged Docker daemon exposed to the internet.

The parameter-context DSP probe (`test-dsp`) remains a different step; a fast
plugin may be incorrect, and a correct plugin may be computationally costly.
