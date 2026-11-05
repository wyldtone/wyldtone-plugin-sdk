import sys
import unittest
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
from check_plugin_configs import inspect

class SDKScaffoldTests(unittest.TestCase):
    def test_abi_header_exists(self):
        self.assertTrue((ROOT / 'abi/include/wasm_plugin.h').is_file())
    def test_regression_configurations_are_valid(self):
        results=inspect()
        self.assertGreaterEqual(len(results), 3)
        self.assertTrue(all(not record['missing'] for record in results))

if __name__ == '__main__': unittest.main()
