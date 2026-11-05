#pragma once

namespace juce
{

class AudioProcessorEditor
{
public:
    explicit AudioProcessorEditor (AudioProcessor&) noexcept {}
    virtual ~AudioProcessorEditor() = default;
};

class GenericAudioProcessorEditor final : public AudioProcessorEditor
{
public:
    explicit GenericAudioProcessorEditor (AudioProcessor& processor)
        : AudioProcessorEditor (processor)
    {
    }
};

} // namespace juce