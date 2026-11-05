# JUCE adapter and licensing

## What is distributed?

The SDK distributes its own JUCE build adapter, interface glue, documentation,
and a small original example source. It does **not** distribute a JUCE checkout,
JUCE's original examples, or prebuilt JUCE-based WAP binaries.

Developers supply a local, unmodified JUCE checkout via `JUCE_DIR`. The currently
validated development baseline is commit `7aae7d8e8` (not a claim of support for
arbitrary JUCE releases). The compiler may generate temporary modified copies
of JUCE files in `build/wyldtone/juce-cmake-build/`; these retain JUCE's rights.

## Separate questions

1. **SDK source:** Original code can be licensed Apache-2.0 by its rights holder.
   This does not grant any license to JUCE or its copyrighted framework.
2. **JUCE example source:** Original example source in this repo is provided
   under the SDK license, **subject to ownership clearance**. Building it links
   JUCE framework code under the developer's chosen JUCE license.
3. **Built WAP:** A JUCE-based WAP may contain AGPL-licensed JUCE code.
   A developer using JUCE under AGPL must fulfill AGPL distribution obligations
   for that combined WAP, including corresponding source and build instructions.
   A source link by itself is not a guarantee of compliance.
4. **Proprietary Wyldtone host:** A public WAP ABI and a separate reference host
   support interoperability. They do **not** conclusively resolve whether a
   particular host/plugin combination forms one copyright work.

Do not imply that a proprietary JUCE-WAP is free to distribute merely because
this SDK is Apache-2.0. Commercial development and AGPL compliance are not
mutually exclusive; details depend on distribution and the applicable rights.

**Marketplace design:** Developers build locally; the marketplace receives
finished WAPs and relevant licensing/source information. The marketplace does
not compile JUCE for uploaders. Handle takedowns and source-code availability
as separate compliance obligations.

This document is general engineering guidance, not legal advice. A specialist
review is recommended before public commercial distribution.
