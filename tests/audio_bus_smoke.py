from pathlib import Path
import json
root=Path(__file__).resolve().parents[1]
cfg=json.loads((root/'examples/gain/plugin.config.json').read_text())
assert cfg['audio']=={'inputs':[2], 'outputs':[2]}
cpp=(root/'crates/wyldtone-cli/src/cpp.rs').read_text()
assert 'generated_bus_source' in cpp and 'wyldtone_generated_buses.cpp' in cpp
main=(root/'crates/wyldtone-cli/src/main.rs').read_text()
assert 'plugin.bus_layout_channels()?' in main
print('Bus metadata static smoke test OK')
