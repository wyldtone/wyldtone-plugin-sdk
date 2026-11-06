# Contributing

Contributions are welcome for the public source-only beta. The SDK is
verified through GitHub Actions on clean hosted runners. Before submitting: `cargo test --release --workspace`, Python unit
tests, and representative Vanilla C++, JUCE (with your own lawful checkout)
and Faust builds. Add regression cases for ABI changes.

Only contribute code you have the right to license under Apache-2.0. Do not
paste JUCE framework source, third-party plugin repositories, or generated
modified JUCE sources into pull requests. By contributing, you represent that
you have the rights needed to submit the code under the project license.
