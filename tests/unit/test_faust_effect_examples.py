"""Lightweight structural checks for the four public Faust effect examples.

This does NOT substitute for compilation by the actual Faust toolchain.
"""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXAMPLES = (
    ("faust_parametric_eq", "parametric_eq.dsp", "EQ"),
    ("faust_bass_mid_drive", "bass_mid_drive.dsp", "Drive"),
    ("faust_stereo_chorus", "stereo_chorus.dsp", "Modulation"),
    ("faust_stereo_slapback", "stereo_slapback.dsp", "Delay"),
)


class FaustEffectsSmokeTests(unittest.TestCase):
    def test_config_schema_and_sources(self):
        ids = set()
        for directory, dsp_name, category in EXAMPLES:
            with self.subTest(example=directory):
                base = ROOT / "examples" / directory
                config = json.loads((base / "plugin.config.json").read_text(encoding="utf-8"))
                self.assertEqual(config["audio"], {"inputs": [2], "outputs": [2]})
                self.assertEqual(config["category"], category)
                self.assertEqual(config["faust"], {"source": dsp_name})
                self.assertNotIn("cpp", config)
                self.assertNotIn("juce", config)
                self.assertNotIn(config["id"], ids)
                ids.add(config["id"])
                dsp = (base / dsp_name).read_text(encoding="utf-8")
                self.assertIn("process =", dsp)
                self.assertIn('declare name "', dsp)
                self.assertIn("hslider(", dsp)
                self.assertIn("SPDX-License-Identifier: Apache-2.0", dsp)

    def test_expected_dsp_building_blocks(self):
        features = {
            "faust_parametric_eq": ("peak_eq_cq", "low_shelf", "high_shelf"),
            "faust_bass_mid_drive": ("crossover2LR4", "tanh", "upperBand"),
            "faust_stereo_chorus": ("fdelay", "oscp", "Stereo Width"),
            "faust_stereo_slapback": ("sdelay", "Stereo Spread", "Tone"),
        }
        for folder, terms in features.items():
            with self.subTest(example=folder):
                files = list((ROOT / "examples" / folder).glob("*.dsp"))
                self.assertEqual(len(files), 1)
                text = files[0].read_text(encoding="utf-8")
                for term in terms:
                    self.assertIn(term, text)


if __name__ == "__main__":
    unittest.main()
