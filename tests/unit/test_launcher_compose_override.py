"""Test SDK launcher Compose configuration selection without Docker."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class LauncherComposeOverrideTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        self.sdk = root / "sdk with spaces"
        self.sdk.mkdir()
        self.launcher = self.sdk / "wyldtone"
        shutil.copy2(ROOT / "wyldtone", self.launcher)
        (self.sdk / "compose.yaml").write_text("services: {}\n")
        fake_bin = root / "fake-bin"
        fake_bin.mkdir()
        self.args_file = root / "docker-args.txt"
        fake_docker = fake_bin / "docker"
        fake_docker.write_text(
            "#!/bin/sh\n"
            'if [ "$1" = "info" ]; then exit 0; fi\n'
            'printf "%s\\n" "$@" > "$CAPTURE_FILE"\n'
        )
        fake_docker.chmod(0o755)
        self.env = dict(os.environ, PATH=f"{fake_bin}:{os.environ['PATH']}",
                        CAPTURE_FILE=str(self.args_file))

    def invoke(self):
        subprocess.run(
            ["bash", str(self.launcher), "doctor"],
            cwd=self.sdk, env=self.env, check=True, capture_output=True, text=True,
        )
        return self.args_file.read_text().splitlines()

    def test_no_override_preserves_base_compose_only(self):
        args = self.invoke()
        self.assertEqual(args.count("-f"), 1)
        self.assertIn(str(self.sdk / "compose.yaml"), args)
        self.assertIn("doctor", args)

    def test_local_override_is_added_in_order(self):
        (self.sdk / "compose.override.yaml").write_text(
            "services:\n  sdk:\n    volumes:\n      - ../wyldtone-plugins:/workspace/projects\n"
        )
        args = self.invoke()
        self.assertEqual(args.count("-f"), 2)
        self.assertLess(args.index(str(self.sdk / "compose.yaml")),
                        args.index(str(self.sdk / "compose.override.yaml")))
        self.assertIn("doctor", args)

    def test_juce_headless_apvts_links_data_structures(self):
        cmake = (ROOT / "adapters/juce/build/CMakeLists.txt").read_text()
        self.assertIn("set(WYLD_JUCE_TARGETS juce::juce_data_structures)", cmake)
        self.assertIn("target_link_libraries(wyldtone_plugin_wasm", cmake)
        self.assertIn("${WYLD_JUCE_TARGETS}", cmake)


if __name__ == "__main__":
    unittest.main()
