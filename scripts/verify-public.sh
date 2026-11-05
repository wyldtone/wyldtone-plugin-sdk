#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 -m unittest discover -s tests/unit -p 'test_*.py'
python3 tests/standalone_smoke.py
python3 tests/audio_bus_smoke.py
python3 tools/check_plugin_configs.py
cargo test --release --locked --workspace
cargo run --release --locked -p wyldtone-cli -- build examples/gain/plugin.config.json
cargo run --release --locked -p wyldtone-wap-host -- render \
  examples/gain/build/wyldtone/SDK-Gain-Example.wap \
  --test-tone 110 --duration 0.5 --out examples/gain/build/wyldtone/reference-output.wav
if command -v faust >/dev/null 2>&1; then
  cargo run --release --locked -p wyldtone-cli -- build examples/faust_gain/plugin.config.json
fi
if [ -n "${JUCE_DIR:-}" ] && [ -d "$JUCE_DIR/modules" ]; then
  cargo run --release --locked -p wyldtone-cli -- build examples/juce_gain/plugin.config.json --skip-dsp-test
fi
