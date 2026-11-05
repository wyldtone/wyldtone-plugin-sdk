#!/usr/bin/env python3
"""Inventory included regression plugin configurations; no engine dependency."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def inspect():
    results = []
    for path in sorted((ROOT / 'examples').glob('*/plugin.config.json')):
        payload = json.loads(path.read_text(encoding='utf-8-sig'))
        required = ('id', 'name', 'manufacturer', 'version')
        missing = [key for key in required if not payload.get(key)]
        results.append({'plugin': path.parent.name, 'backend': 'juce' if 'juce' in payload else ('cpp' if 'cpp' in payload else ('faust' if 'faust' in payload else 'unknown')), 'missing': missing})
    return results

if __name__ == '__main__':
    results = inspect()
    for item in results:
        print(f"{item['plugin']}: {item['backend']}" + (f" MISSING {item['missing']}" if item['missing'] else ''))
    if not results or any(item['missing'] for item in results):
        raise SystemExit(1)
