"""Public-release static boundary checks (no compiler required)."""
import unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]

class ReleaseBoundaryTests(unittest.TestCase):
    def test_private_sources_not_shipped(self):
        for name in ('migration', 'target', 'wasm-cache', 'tests/fixtures', 'adapters/juce/patches'):
            self.assertFalse((ROOT/name).exists(), name)
    def test_host_is_offline_and_independent(self):
        host=(ROOT/'crates/wyldtone-wap-host/src/main.rs').read_text()
        self.assertIn('--test-tone',host)
        self.assertIn('--wav',host)
        self.assertIn('--out',host)
        self.assertNotIn('audio_engine::',host)
    def test_required_release_docs(self):
        for path in ('LICENSE','NOTICE','THIRD-PARTY-NOTICES.md','SECURITY.md',
                    'docs/juce-licensing.md','docs/wap-format.md','docs/reference-host.md'):
            self.assertTrue((ROOT/path).is_file(),path)
    def test_no_distributed_prebuilt_plugins(self):
        for suffix in ('*.wap','*.wasm'):
            self.assertEqual(list(ROOT.rglob(suffix)), [])

if __name__=='__main__':unittest.main()
