"""Source-level regression checks; no Faust/Emscripten toolchain needed."""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class FaustYinHybridTest(unittest.TestCase):
    def test_declared_layout(self):
        cfg = json.loads((ROOT / "examples/faust_bass_synth/plugin.config.json").read_text())
        self.assertEqual(cfg["audio"], {"inputs": [2], "outputs": [2]})
        self.assertEqual(cfg["faust"]["extra_inputs"], 2)
        for source in cfg["faust"]["extra_sources"]:
            self.assertTrue((ROOT / "examples/faust_bass_synth" / source).is_file())

    def test_internal_control_channels(self):
        dsp = (ROOT / "examples/faust_bass_synth/bass_synth.dsp").read_text()
        self.assertIn("process(left, right, yinPitchHz, yinNoteGate)", dsp)
        self.assertNotIn("an.pitchTracker(", dsp)
        adapter = (ROOT / "adapters/faust/wyldtone_bridge.cpp").read_text()
        self.assertIn("wyldtone_faust_extra_process", adapter)
        self.assertIn("return WYLDTONE_FAUST_MAIN_INPUTS;", adapter)

    def test_back_compat_default(self):
        cfg = (ROOT / "crates/wyldtone-plugin-api/src/config.rs").read_text()
        self.assertIn("pub extra_inputs: u32", cfg)
        self.assertIn("pub extra_sources: Vec<String>", cfg)
        self.assertIn("#[serde(default)]", cfg)


if __name__ == "__main__":
    unittest.main()
