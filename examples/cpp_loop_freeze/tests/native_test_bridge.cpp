// Compiled only in native tests, uses the same internal DSP path as WASM.
#include "../plugin.cpp"
void nativeTestProcess(float* buf, int frames) { processInterleaved(buf, frames); }
