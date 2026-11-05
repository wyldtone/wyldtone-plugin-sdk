# Optional remote Linux build host

Clone this public SDK onto an x86-64 Linux machine. Build using the bundled
Compose setup (`docker compose build sdk`) and the quick-start commands in
`docker/README.md`. This does not require WSL on Windows development machines.

The environment pins its Emscripten base and Rust toolchain, but the Debian
Faust package and transitive packages are **not** independently pinned. Record
versions with `docker compose run --rm sdk versions`, and lock image digests
before publishing reproducible CI artifacts.

For JUCE builds developers obtain their own clean checkout and set it at
`../JUCE-upstream`. No JUCE source is included in this SDK distribution.

Build caches persist under `.cache/`, which is excluded from version control.
Avoid running untrusted WAPs on shared or privileged infrastructure.
