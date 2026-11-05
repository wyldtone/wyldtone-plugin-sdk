import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class FaustSdkLayoutTests(unittest.TestCase):
    def test_example_selects_only_faust(self):
        conf = json.loads((ROOT / 'examples/faust_gain/plugin.config.json').read_text())
        self.assertIn('faust', conf)
        self.assertNotIn('cpp', conf)
        self.assertNotIn('juce', conf)
        self.assertEqual(conf['audio'], {'inputs': [2], 'outputs': [2]})
        self.assertTrue((ROOT / 'examples/faust_gain' / conf['faust']['source']).is_file())

    def test_architecture_has_faust_insertion_points(self):
        arch = (ROOT / 'adapters/faust/wyldtone_arch.cpp').read_text()
        self.assertIn('<<includeIntrinsic>>', arch)
        self.assertIn('<<includeclass>>', arch)

    def test_abi_and_package_pipeline_reused(self):
        rust = (ROOT / 'crates/wyldtone-cli/src/faust.rs').read_text()
        self.assertIn('cpp::build(', rust)
        self.assertIn('WyldFaustDSP', rust)
        main = (ROOT / 'crates/wyldtone-cli/src/main.rs').read_text()
        self.assertIn('faust::build(', main)
        self.assertIn('generate_manifest(', main)
        self.assertIn('create_wap_with_icon(', main)


if __name__ == '__main__':
    unittest.main()
