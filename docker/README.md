# Docker developer environment

On a Linux/amd64 host with Docker Compose:

```sh
mkdir -p .cache/cargo .cache/target
sudo docker compose build sdk
sudo docker compose run --rm sdk versions
sudo docker compose run --rm sdk test
sudo docker compose run --rm sdk build examples/gain/plugin.config.json
sudo docker compose run --rm sdk build examples/faust_gain/plugin.config.json
sudo docker compose run --rm sdk host render \
  examples/gain/build/wyldtone/SDK-Gain-Example.wap \
  --test-tone 110 --duration 1 --out examples/gain/build/wyldtone/result.wav
```

See `docs/reference-host.md`. The Docker image uses an external JUCE checkout
mounted read-only at `/opt/juce` if it exists. The image doesn't redistribute
JUCE and the engine is not part of this repository.

On shared machines limit build jobs and container resources; the included
Compose currently specifies a PID limit and drops Linux capabilities,
but it does **not** impose a CPU or memory quota. Limit CPU/memory
explicitly if needed on shared build machines. The `host` subcommand is an
offline local test tool, not a hardened marketplace-execution sandbox.
