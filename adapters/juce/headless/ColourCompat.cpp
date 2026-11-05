// Wyldtone SDK - minimal JUCE Colour compatibility for headless WASM.
//
// JUCE's juce_audio_processors_headless deliberately avoids linking
// juce_graphics. Some third-party processors nevertheless instantiate
// juce::Colour constants in non-editor translation units.
//
// Supply ONLY the ARGB constructor required by these constants. This is
// not a substitute for juce_graphics, and must not be linked together with
// the actual juce_graphics implementation of Colour.
//
// This file is SDK-owned. The developer's JUCE checkout remains untouched.
#include <juce_graphics/juce_graphics.h>

#if ! JUCE_WASM
 #error "Wyldtone ColourCompat is exclusively for headless JUCE/WASM builds"
#endif

namespace juce
{
Colour::Colour (uint32 argbValue) noexcept
    : argb (static_cast<uint8> ((argbValue >> 24) & 0xff),
            static_cast<uint8> ((argbValue >> 16) & 0xff),
            static_cast<uint8> ((argbValue >> 8) & 0xff),
            static_cast<uint8> (argbValue & 0xff))
{
}
} // namespace juce
