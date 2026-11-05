# Audio bus metadata

The standalone v0.2 Vanilla C++ builder generates the four optional WASM
bus exports for plugins with an `audio` entry. A plugin lacking this entry
retains its old exports and legacy fallback behavior.

Examples:
- `{"inputs":[1],"outputs":[1]}`: mono
- `{"inputs":[2],"outputs":[2]}`: stereo
- `{"inputs":[1],"outputs":[2]}`: mono-in stereo-out
- `{"inputs":[2,1],"outputs":[2]}`: stereo + mono sidechain

Only bus shapes actually supported by the plugin's audio processing code
should be configured. The validator checks compiled exported metadata,
not signal correctness. Inputs and outputs must have at most 32 buses with
1..64 channels per bus. The host decides which layouts can be connected.

The v0.2 SDK does not change the engine, add upmix/downmix, or automatically
support multi-layout plugins. A JUCE integration will need to translate its
supported BusesProperties/layout into the same ABI in a later step.
