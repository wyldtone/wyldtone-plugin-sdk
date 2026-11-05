# Original JUCE Gain example

This intentionally small example is SDK-authored and does **not** contain JUCE
framework source code. It needs a developer-supplied JUCE checkout for building.

```sh
docker compose run --rm sdk build examples/juce_gain/plugin.config.json
```

The compiled WAP includes JUCE library code and is subject to the selected JUCE
licence. The source files alone are intended to be covered by Apache-2.0 after
rights confirmation; see `docs/juce-licensing.md`. No compiled WAP is included.
