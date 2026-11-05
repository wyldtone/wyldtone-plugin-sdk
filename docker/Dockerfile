# Wyldtone SDK build container - initial Linux/amd64 validation.
# Pin the published Emscripten and Rust toolchains; no JUCE sources in image.
FROM emscripten/emsdk:6.0.13

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:${PATH}

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates curl git cmake ninja-build build-essential pkg-config \
       python3 faust \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain 1.99.1 \
    && chmod -R a+rX /usr/local/cargo /usr/local/rustup

COPY docker/entrypoint.sh /usr/local/bin/wyldtone-sdk
RUN chmod 755 /usr/local/bin/wyldtone-sdk

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/wyldtone-sdk"]
CMD ["help"]
