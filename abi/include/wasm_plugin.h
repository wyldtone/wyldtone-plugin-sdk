#pragma once

#include <stdint.h>
#include <emscripten/emscripten.h>

/*
 * ============================================================================
 *  WASM Plugin ABI
 * ============================================================================
 *
 *  This header describes the ABI between a native C/C++ plugin compiled to
 *  standalone WebAssembly and the Rust host.
 *
 *  The host-visible ABI is deliberately kept simple:
 *
 *      - primitive integer / floating point values
 *      - pointers into WASM linear memory
 *      - explicit buffers
 *
 *  No C++ objects, STL containers or platform-specific types may cross the
 *  WASM boundary directly.
 *
 *  Plugins may implement only the optional capabilities they actually need.
 *
 *  In particular, the opaque plugin state API below is OPTIONAL.
 *  Plugins whose complete state is represented by host-visible parameters do
 *  not need to implement the state functions at all.
 */


/*
 * ============================================================================
 *  Export helper
 * ============================================================================
 *
 *  Optional ABI functions cannot be listed globally in Emscripten's
 *  EXPORTED_FUNCTIONS setting, because not every plugin implements them.
 *
 *  WASM_PLUGIN_EXPORT uses EMSCRIPTEN_KEEPALIVE so that:
 *
 *      1. Emscripten does not remove the function during dead-code elimination.
 *      2. The function is exported from the final WASM module.
 *
 *  Use this macro on every optional ABI function implemented by the plugin.
 *
 *  Example:
 *
 *      WASM_PLUGIN_EXPORT
 *      uint32_t plugin_state_version()
 *      {
 *          return 1;
 *      }
 */

#define WASM_PLUGIN_EXPORT EMSCRIPTEN_KEEPALIVE


/*
 * ============================================================================
 *  Plugin events
 * ============================================================================
 *
 *  plugin_pending_events() returns a bit mask describing events that occurred
 *  inside the plugin since the previous query.
 *
 *  Multiple events may be combined using bitwise OR.
 */


/*
 * A host-visible parameter changed inside the plugin.
 *
 * Example:
 *
 *      return WASM_PLUGIN_EVENT_PARAMETER_CHANGED;
 */
#define WASM_PLUGIN_EVENT_PARAMETER_CHANGED 0x01


/*
 * Internal plugin state changed.
 *
 * This event does NOT ask the host to immediately serialize the plugin.
 *
 * It only informs the host that its cached opaque state is now outdated.
 *
 * The host may defer plugin_state_capture() until an explicit save operation.
 *
 * This is important because serialization may be expensive and should not be
 * performed as part of the regular real-time audio processing path.
 */
#define WASM_PLUGIN_EVENT_STATE_CHANGED 0x02


#ifdef __cplusplus
extern "C" {
#endif


/*
 * ============================================================================
 *  Optional audio bus-layout metadata API
 * ============================================================================
 *
 *  Plugins may expose these four functions to describe their audio buses to
 *  the host before audio processing is prepared. Older plugins that do not
 *  implement this API are currently treated as legacy mono plugins.
 *
 *  If bus metadata is implemented, ALL FOUR functions must be implemented.
 *
 *  For ordinary C/C++ plugins these are optional exports, so implementations
 *  must use WASM_PLUGIN_EXPORT. Example:
 *
 *      WASM_PLUGIN_EXPORT
 *      int32_t plugin_input_bus_count(void)
 *      {
 *          return 1;
 *      }
 *
 *      WASM_PLUGIN_EXPORT
 *      int32_t plugin_input_bus_channels(int32_t bus_index)
 *      {
 *          return bus_index == 0 ? 2 : 0;
 *      }
 *
 *  A conventional stereo effect therefore reports one input bus with two
 *  channels and one output bus with two channels.
 */

int32_t plugin_input_bus_count(void);
int32_t plugin_output_bus_count(void);
int32_t plugin_input_bus_channels(int32_t bus_index);
int32_t plugin_output_bus_channels(int32_t bus_index);


/*
 * ============================================================================
 *  Optional opaque plugin state API
 * ============================================================================
 *
 *  This complete section is OPTIONAL.
 *
 *  A plugin only needs to implement these functions if it maintains internal
 *  state that cannot be reconstructed entirely from the parameters known to
 *  the host.
 *
 *  Examples of opaque state:
 *
 *      - loaded IR data
 *      - loaded neural network/model data
 *      - modulation matrices
 *      - MIDI mappings
 *      - internal routing
 *      - plugin-specific configuration
 *      - additional data structures not exposed as parameters
 *
 *  If a plugin supports opaque state, it should implement ALL functions in
 *  this section.
 *
 *  If it does not support opaque state, it should implement NONE of them.
 *
 *
 *  Typical save flow:
 *
 *      Host
 *        |
 *        | plugin_state_capture()
 *        v
 *      Plugin serializes internal state
 *        |
 *        | returns byte size
 *        v
 *      Host
 *        |
 *        | plugin_state_buffer_ptr()
 *        v
 *      Host copies the serialized bytes out of WASM memory
 *
 *
 *  Typical load flow:
 *
 *      Host
 *        |
 *        | plugin_state_prepare_load(size)
 *        v
 *      Plugin allocates/prepares writable buffer
 *        |
 *        | returns pointer
 *        v
 *      Host writes state bytes into WASM memory
 *        |
 *        | plugin_state_commit_load(size)
 *        v
 *      Plugin deserializes and applies state
 */


/*
 * Returns the version of the serialized opaque state format.
 *
 * IMPORTANT:
 *
 * This is NOT the plugin version.
 *
 * Example:
 *
 *      Plugin version:       2.7.3
 *      State format version: 1
 *
 * The plugin may evolve through many releases without changing its serialized
 * state format.
 *
 * Increment this value only when the serialized state representation changes
 * incompatibly.
 *
 * The host stores this value together with the serialized state so future
 * versions of the plugin can decide whether migration is required.
 *
 * Returns:
 *
 *      State format version >= 1
 */
uint32_t plugin_state_version(void);


/*
 * Serializes the current opaque plugin state.
 *
 * The plugin should serialize its current internal state into a buffer owned
 * by the plugin.
 *
 * After this call succeeds, plugin_state_buffer_ptr() must return a pointer to
 * that serialized buffer.
 *
 * The returned buffer must remain valid until at least the next state-related
 * API call.
 *
 * This operation MAY be expensive.
 *
 * The host therefore must not assume that it is safe to call this function
 * regularly from the real-time audio path.
 *
 * Returns:
 *
 *      >= 0    Number of serialized bytes.
 *
 *       0      Valid empty state.
 *
 *      < 0     Error.
 *
 * Recommended initial convention:
 *
 *      -1      Generic serialization error.
 *
 * Additional negative error codes may be defined in future ABI versions.
 */
int32_t plugin_state_capture(void);


/*
 * Returns a pointer to the serialized state buffer produced by the most recent
 * successful plugin_state_capture() call.
 *
 * The pointer refers to an offset inside the module's WASM linear memory.
 *
 * The host will copy plugin_state_capture() bytes starting at this address.
 *
 * Returns:
 *
 *      > 0     Pointer / linear-memory offset of the serialized state buffer.
 *
 *        0     Valid when the serialized state size is zero.
 *
 * Notes:
 *
 *      - The pointer must remain valid until the next state-related function
 *        call.
 *
 *      - The host never takes ownership of this memory.
 *
 *      - The plugin may reuse the buffer after the host has copied it.
 */
int32_t plugin_state_buffer_ptr(void);


/*
 * Prepares a writable buffer for loading an opaque plugin state.
 *
 * The host calls this before copying serialized state bytes into WASM memory.
 *
 * Parameters:
 *
 *      size    Number of bytes the host intends to write.
 *
 * The plugin must allocate or resize an internal buffer so that at least
 * `size` bytes can be written safely.
 *
 * The returned pointer refers to writable WASM linear memory.
 *
 * Returns:
 *
 *      > 0     Pointer / linear-memory offset of the writable state buffer.
 *
 *        0     Valid when size == 0.
 *              May also be used to indicate failure for size > 0.
 *
 * After receiving the pointer, the host writes exactly `size` bytes into the
 * buffer and then calls plugin_state_commit_load(size).
 */
int32_t plugin_state_prepare_load(int32_t size);


/*
 * Applies a state previously written by the host.
 *
 * This function is called after:
 *
 *      1. plugin_state_prepare_load(size)
 *      2. the host copied `size` bytes into the returned buffer
 *
 * The plugin should now deserialize the buffer and update its internal state.
 *
 * Parameters:
 *
 *      size    Number of valid bytes written by the host.
 *
 * A size of zero represents a valid empty state.
 *
 * The implementation should validate the input before applying it.
 *
 * The function deliberately returns void in ABI v1. If explicit load errors
 * need to be reported in the future, the ABI can be extended in a backwards-
 * compatible way.
 */
void plugin_state_commit_load(int32_t size);


/* Optional multi-bus processing ABI (v2).
 * Input: sample_count frames, each containing all input channels in bus order.
 * Output: immediately after the input region, sample_count frames of all
 * output channels in bus order. The layout is described by bus metadata.
 * The caller must allocate (input_channels + output_channels) * sample_count
 * floats at plugin_buffer_ptr(). Legacy plugin_process remains supported.
 */
void plugin_process_buses(int32_t ptr, int32_t sample_count);

#ifdef __cplusplus
} // extern "C"
#endif
