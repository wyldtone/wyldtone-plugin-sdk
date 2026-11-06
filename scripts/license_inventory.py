#!/usr/bin/env python3
"""Report all resolved Rust package license declarations from cargo metadata.

Input: cargo metadata --format-version 1 --locked (JSON on stdin)
Output: machine-readable TSV and human-readable review exceptions.
This is inventory, NOT a legal certification; inspect LICENSE/NOTICE texts.
"""
import csv
import json
import re
import sys

PERMISSIVE = {
    "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC",
    "Zlib", "0BSD", "BSL-1.0", "CC0-1.0", "Unlicense", "MIT-0",
    "Unicode-DFS-2016", "Unicode-3.0", "NCSA", "OFL-1.1",
}
REVIEW = {"MPL-2.0", "OpenSSL", "CDLA-Permissive-2.0"}
LICENSE_PATTERN = re.compile(r"\\b(?:AND|OR|WITH)\\b|[()]|[^\\s()]+")


def classify(expr):
    if not expr:
        return "REVIEW: unspecified SPDX expression"
    # Metadata is a declaration, not proof of license authenticity.
    tokens = LICENSE_PATTERN.findall(expr.replace("/", " OR "))
    lic = []
    i = 0
    while i < len(tokens):
        t = tokens[i]
        if t in ("AND", "OR", "(", ")"):
            i += 1
            continue
        if i + 2 < len(tokens) and tokens[i+1] == "WITH":
            t += " WITH " + tokens[i+2]
            i += 3
        else:
            i += 1
        lic.append(t)
    if not lic:
        return "REVIEW: malformed license"
    if any(x.startswith(("GPL-", "AGPL-", "LGPL-", "SSPL-")) for x in lic):
        return "REVIEW: copyleft"
    if any(x in REVIEW for x in lic):
        return "REVIEW: special obligations"
    if all(x in PERMISSIVE or x == "Apache-2.0 WITH LLVM-exception" for x in lic):
        return "DECLARED PERMISSIVE"
    return "REVIEW: unknown identifier"


def main():
    data = json.load(sys.stdin)
    packages = sorted(data["packages"], key=lambda p: (p["name"], p["version"]))
    external = [p for p in packages if p.get("source")]
    print(f"Resolved packages: {len(packages)}; external: {len(external)}; workspace: {len(packages)-len(external)}")
    print("NAME\\tVERSION\\tLICENSE_EXPRESSION\\tLICENSE_FILE\\tCLASSIFICATION")
    review = []
    for p in external:
        expr = p.get("license") or ""
        status = classify(expr)
        print("\\t".join((p["name"], p["version"], expr or "(unspecified)", p.get("license_file") or "", status)))
        if status.startswith("REVIEW"):
            review.append((p["name"], p["version"], expr, status))
    print(f"SUMMARY: {len(external)-len(review)} external package declarations permissive; {len(review)} require manual review", file=sys.stderr)
    for name, version, expr, status in review:
        print(f"MANUAL REVIEW: {name} {version}: {expr or '(none)'} -- {status}", file=sys.stderr)
    # Inventory is informational; flag exceptions in log without breaking beta CI.
    return 0


if __name__ == "__main__":
    sys.exit(main())
