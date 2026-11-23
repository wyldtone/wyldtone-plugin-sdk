# WASM processor latency reporting

The optional exports `plugin_latency_samples()` and
`plugin_max_latency_samples()` report **whole host-rate audio frames**,
not CPU time, buffering of the host's audio driver or musical echo/pre-delay.

## Examples

An entirely sample-synchronous effect may omit both exports. The host then
assumes zero latency (the historical ABI behavior).

A fixed-latency spectral processor may export:

```cpp
WASM_PLUGIN_EXPORT int32_t plugin_latency_samples() { return 511; }
WASM_PLUGIN_EXPORT int32_t plugin_max_latency_samples() { return 511; }
```

A variable-quality oversampled processor should report its **currently
selected** latency and a worst-case bound over all quality settings. For
example, a causal FIR processor with 0/31/46 host-frame delays might expose:

```cpp
WASM_PLUGIN_EXPORT int32_t plugin_latency_samples() {
    return current_quality == 4 ? 46 : current_quality == 2 ? 31 : 0;
}
WASM_PLUGIN_EXPORT int32_t plugin_max_latency_samples() { return 46; }
```

The maximum is read after `plugin_init` during graph preparation and must
remain valid for the prepared sample rate. The host reserves any parallel-path
compensation buffers outside the audio processing callback. The current
latency may change only within this bound. Parameter updates trigger a host
refresh automatically; internal quality changes should also set event bit
`WASM_PLUGIN_EVENT_LATENCY_CHANGED` (0x08) in
`plugin_pending_events()`.

Both exports are optional and are independent of state serialization and
licensing. Exported functions must be cheap, deterministic and allocation-free.
Current ABI version has no fractional-frame delay; choose filter lengths that
yield integral end-to-end latency.

**Do not report musical delays as processor latency.** Chorus, echo, reverb
pre-delay and a dry+wet mixer with unaligned paths generally do not have a
single meaningful effective latency. Where a dry-alignment mode is available,
the plugin may report its delay only in that mode, provided it accurately
represents all of its output components.

Older WAPs still load; older hosts simply ignore the new exports.