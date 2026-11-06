#!/usr/bin/env python3
"""Fail CI if Git-tracked source-only SDK files contain build outputs/secrets.

Run from any directory. This inspects the Git index, not .gitignore, because
ignore patterns do not protect files that have already been committed.
"""
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SUFFIXES = {
    ".wap", ".wasm", ".wav", ".nam", ".zip", ".7z", ".tgz", ".tar",
    ".exe", ".dll", ".so", ".dylib", ".o", ".a", ".pyc", ".log",
    ".pem", ".key", ".p12", ".pfx", ".kdbx", ".sqlite", ".sqlite3", ".db",
}
FORBIDDEN_PARTS = {
    "target", "wasm-cache", "__pycache__", ".cache",
    "JUCE-upstream", "JUCE", "thirdparty", "migration",
}
FORBIDDEN_NAMES = {"id_rsa", "id_ed25519"}
MAX_TRACKED_FILE = 4 * 1024 * 1024  # Source-only beta; review exceptions.

result = subprocess.run(
    ["git", "ls-files", "--cached", "-z"], cwd=ROOT,
    check=True, capture_output=True,
)
files = [
    Path(name.decode("utf-8", "surrogateescape"))
    for name in result.stdout.split(b"\x00") if name
]
violations = []
for relative in files:
    path = ROOT / relative
    parts = relative.parts
    filename = relative.name
    # A path in the Git index may be absent in a partially checked-out tree.
    if not path.is_file():
        violations.append(f"{relative}: tracked file missing from checkout")
        continue
    if any(part in FORBIDDEN_PARTS for part in parts):
        violations.append(f"{relative}: forbidden generated/vendor directory")
    # The checked-in JUCE CMake adapter lives in adapters/juce/build/.
    # Every other build/ directory is generated output.
    if "build" in parts and parts[:3] != ("adapters", "juce", "build"):
        violations.append(f"{relative}: generated build output")
    if filename in FORBIDDEN_NAMES or filename.startswith(("id_rsa.", "id_ed25519.")):
        violations.append(f"{relative}: private key filename")
    if filename == ".env" or (filename.startswith(".env.") and filename != ".env.example"):
        violations.append(f"{relative}: private environment configuration")
    if relative.suffix.lower() in FORBIDDEN_SUFFIXES or filename.lower().endswith((".tar.gz", ".tar.xz")):
        violations.append(f"{relative}: unexpected binary/archive/credential extension")
    if path.stat().st_size > MAX_TRACKED_FILE:
        violations.append(f"{relative}: file exceeds 4 MiB source-review limit")

if violations:
    print("Source-only release inventory FAILED:")
    for issue in violations:
        print(" -", issue)
    sys.exit(1)
print(f"PASS: {len(files)} Git-tracked files; no prohibited build outputs, archives, credentials or oversized files")
