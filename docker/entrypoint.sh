#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -eq 0 ]; then
  set -- help
fi

case "$1" in
  new|doctor|inspect|validate|build|manifest|package|test-dsp|benchmark)
    exec cargo run --release --locked -p wyldtone-cli -- "$@"
    ;;
  host)
    shift
    exec cargo run --release --locked -p wyldtone-wap-host -- "$@"
    ;;
  test)
    shift
    exec cargo test --release --locked --workspace "$@"
    ;;
  cargo)
    shift
    exec cargo "$@"
    ;;
  shell)
    shift
    exec bash "$@"
    ;;
  versions)
    echo "Emscripten:"; em++ --version | head -n 1
    echo "Rust:"; rustc --version
    echo "Cargo:"; cargo --version
    echo "CMake:"; cmake --version | head -n 1
    echo "Ninja:"; ninja --version
    echo "Faust:"; faust -v 2>&1 | head -n 1
    ;;
  help)
    cat <<'HELP'
Usage:
  docker compose run --rm sdk versions
  docker compose run --rm sdk doctor
  docker compose run --rm sdk new MyEffect --template cpp
  docker compose run --rm sdk new MySynth --template faust
  docker compose run --rm sdk new MyJucePlugin --template juce
  docker compose run --rm sdk build MyEffect
  docker compose run --rm sdk inspect MyEffect/build/wyldtone/MyEffect.wap
  docker compose run --rm sdk validate MyEffect/build/wyldtone/MyEffect.wap
  docker compose run --rm sdk build examples/gain/plugin.config.json
  docker compose run --rm sdk build examples/faust_gain/plugin.config.json
  docker compose run --rm sdk test-dsp examples/gain/build/wyldtone/plugin.wasm
  docker compose run --rm sdk benchmark examples/gain/build/wyldtone/plugin.wasm --reference-id local-reference-v1
  docker compose run --rm sdk test
  docker compose run --rm sdk host inspect examples/gain/build/wyldtone/SDK-Gain-Example.wap
  docker compose run --rm sdk host render examples/gain/build/wyldtone/SDK-Gain-Example.wap --test-tone 110 --out test.wav
  docker compose run --rm sdk shell
HELP
    ;;
  *)
    exec "$@"
    ;;
esac
