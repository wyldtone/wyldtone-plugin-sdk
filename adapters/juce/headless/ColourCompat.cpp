// Wyldtone SDK: headless WebAssembly adapter for JUCE's declared Colour API.
//
// This is an independently authored, minimal constructor shim. It uses the
// public PixelARGB setter instead of carrying over the JUCE Colour.cpp
// constructor implementation. The real juce_graphics implementation MUST NOT
// be linked alongside this shim.
//
// No JUCE source files are vendored or redistributed by this file.
#include <juce_graphics/juce_graphics.h>

#if ! JUCE_WASM
 #error "Wyldtone ColourCompat is exclusively for headless JUCE/WASM builds"
#endif

namespace juce
{
Colour::Colour (uint32 packedColour) noexcept
{
    constexpr uint32 componentMask = 255u;
    constexpr uint32 redOffset = 16u;
    constexpr uint32 greenOffset = 8u;
    constexpr uint32 alphaOffset = 24u;

    // The ARGB byte layout is part of the published Colour(uint32) API.
    // Use PixelARGB's public setter to keep this shim independent from
    // JUCE's implementation of Colour's own constructor.
    const auto channel = [packedColour] (uint32 offset) noexcept
    {
        return static_cast<uint8> ((packedColour >> offset) & componentMask);
    };

    argb.setARGB (channel (alphaOffset),
                  channel (redOffset),
                  channel (greenOffset),
                  channel (0u));
}
} // namespace juce
