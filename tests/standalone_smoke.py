"""Source-level smoke test; no external Rust/emsdk required."""
from pathlib import Path
import json
root=Path(__file__).resolve().parents[1]
assert json.loads((root/'examples/gain/plugin.config.json').read_text())['cpp']['sources']==['plugin.cpp']
assert (root/'abi/include/wasm_plugin.h').exists()
assert 'audio_engine::' not in (root/'crates/wyldtone-cli/src/main.rs').read_text()
assert 'audio_engine::' not in (root/'crates/wyldtone-plugin-runtime/src/wasm_abi.rs').read_text()
print('Static SDK smoke checks passed')
