// Minimal Faust architecture for the Wyldtone WASM SDK.
// Faust expands the two placeholders below and, with -i, embeds required
// Faust architecture headers into the generated translation unit.
#ifndef FAUSTFLOAT
#define FAUSTFLOAT float
#endif
#include "faust/dsp/dsp.h"
#include "faust/gui/UI.h"
#include "faust/gui/meta.h"

<<includeIntrinsic>>
<<includeclass>>
