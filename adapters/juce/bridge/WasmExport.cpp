#if defined(WYLD_JUCE_FULL)
#include <juce_audio_processors/juce_audio_processors.h>
#else
#include <juce_audio_processors_headless/juce_audio_processors_headless.h>
#endif

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstring>
#include <memory>
#include <vector>

using namespace juce;

// Provided by the plugin-specific entry point.
std::unique_ptr<AudioProcessor> createPlugin();

namespace
{
    constexpr uint32_t MAX_BUFFER_SAMPLES = 16384;
    constexpr int32_t MAX_STRING_SIZE = 256;

    constexpr uint8_t EVENT_PARAMETER_CHANGED = 0x01;
    constexpr uint8_t EVENT_STATE_CHANGED     = 0x02;

    std::atomic<uint8_t> pendingEvents{0};

    std::unique_ptr<AudioProcessor> processor;

    // Interleaved WASM/ABI buffer.
    //
    // This is the memory exposed through plugin_buffer_ptr().
    std::vector<float> wasmBuffer(MAX_BUFFER_SAMPLES);

    // Opaque plugin state is captured only on explicit save/load paths.
    // Keeping the buffer inside WASM lets the host copy bytes without
    // exposing an allocator in the plugin ABI.
    std::vector<uint8_t> stateBuffer;

    // JUCE uses a non-interleaved AudioBuffer.
    AudioBuffer<float> audioBuffer;

    MidiBuffer midiBuffer;

    int32_t currentSampleRate = 0;
    int32_t currentChannels = 0;
    int32_t currentBlockSize = 0;

    bool initialized = false;

    class PluginEventListener
        : public AudioProcessorListener
    {
    public:
        void audioProcessorParameterChanged(
            AudioProcessor* /*processor*/,
            int /*parameterIndex*/,
            float /*newValue*/) override
        {
            pendingEvents.fetch_or(
                EVENT_PARAMETER_CHANGED,
                std::memory_order_release);
        }

        void audioProcessorChanged(
            AudioProcessor* /*processor*/,
            const ChangeDetails& details) override
        {
            if (details.nonParameterStateChanged)
            {
                pendingEvents.fetch_or(
                    EVENT_STATE_CHANGED,
                    std::memory_order_release);
            }
        }
    };

    PluginEventListener eventListener;

    // -------------------------------------------------------------------------
    // Parameter helpers
    // -------------------------------------------------------------------------

    AudioProcessorParameter* getParameter(int32_t index)
    {
        if (!processor || index < 0)
            return nullptr;

        const auto& parameters = processor->getParameters();

        if (index >= parameters.size())
            return nullptr;

        return parameters[index];
    }


    RangedAudioParameter* getRangedParameter(int32_t index)
    {
        return dynamic_cast<RangedAudioParameter*>(getParameter(index));
    }


    const NormalisableRange<float>* getParameterRange(
        int32_t index)
    {
        auto* parameter = getRangedParameter(index);

        if (!parameter)
            return nullptr;

        return &parameter->getNormalisableRange();
    }


    // -------------------------------------------------------------------------
    // String helper
    // -------------------------------------------------------------------------

    int32_t copyString(
        const String& text,
        int32_t bufferPtr,
        int32_t bufferSize)
    {
        if (bufferPtr < 0 || bufferSize <= 0)
            return 0;

        const auto utf8 = text.toRawUTF8();
        const auto length =
            static_cast<int32_t>(std::strlen(utf8));

        const auto copyLength =
            std::min(length, bufferSize);

        auto* destination =
            reinterpret_cast<char*>(
                static_cast<uintptr_t>(bufferPtr));

        std::memcpy(
            destination,
            utf8,
            static_cast<size_t>(copyLength));

        return copyLength;
    }


    // -------------------------------------------------------------------------
    // Parameter type detection
    // -------------------------------------------------------------------------

    enum ParameterFlags : int32_t
    {
        FLAG_AUTOMATABLE = 1,
        FLAG_INT         = 2,
        FLAG_BOOL        = 4,
        FLAG_CHOICE      = 8
    };


    int32_t getParameterFlags(int32_t index)
    {
        auto* parameter = getParameter(index);

        if (!parameter)
            return 0;

        int32_t flags = 0;

        if (parameter->isAutomatable())
            flags |= FLAG_AUTOMATABLE;

        if (dynamic_cast<AudioParameterBool*>(parameter))
        {
            flags |= FLAG_BOOL;
            return flags;
        }

        if (dynamic_cast<AudioParameterChoice*>(parameter))
        {
            flags |= FLAG_CHOICE;
            return flags;
        }

        if (dynamic_cast<AudioParameterInt*>(parameter))
        {
            flags |= FLAG_INT;
            return flags;
        }

        return flags;
    }
}


// =============================================================================
// WASM ABI
// =============================================================================

extern "C"
{

// -----------------------------------------------------------------------------
// Lifecycle
// -----------------------------------------------------------------------------

void plugin_create()
{
    processor = createPlugin();

    if (!processor)
        return;

    processor->addListener(&eventListener);

    pendingEvents.store(
        0,
        std::memory_order_release);

    initialized = false;
}


void plugin_init(
    int32_t sample_rate,
    int32_t channels,
    int32_t block_size)
{
    if (sample_rate <= 0 ||
        channels <= 0 ||
        block_size <= 0)
    {
        return;
    }

    if (!processor)
        return;

    currentSampleRate = sample_rate;
    currentChannels = channels;
    currentBlockSize = block_size;

    const auto juceChannels = juce::jmax(
        processor->getTotalNumInputChannels(),
        processor->getTotalNumOutputChannels());

    audioBuffer.setSize(
        juce::jmax(channels, juceChannels),
        0,
        false,
        false,
        true);

    midiBuffer.clear();

    // JUCE-internen Zustand für Sample-Rate und Blockgröße setzen.
    processor->setRateAndBufferSizeDetails(
        static_cast<double>(sample_rate),
        block_size);

    // Anschließend das Plugin wie gewohnt vorbereiten.
    processor->prepareToPlay(
        static_cast<double>(sample_rate),
        block_size);

    initialized = true;
}

void plugin_reset()
{
    if (!processor)
        return;

    processor->reset();

    initialized = false;
}

uint8_t plugin_pending_events()
{
    return pendingEvents.exchange(
        0,
        std::memory_order_acquire);
}


// -----------------------------------------------------------------------------
// Audio bus layout
// -----------------------------------------------------------------------------

int32_t plugin_input_bus_count()
{
    if (!processor)
        return 0;

    return processor->getBusCount(true);
}

int32_t plugin_output_bus_count()
{
    if (!processor)
        return 0;

    return processor->getBusCount(false);
}

int32_t plugin_input_bus_channels(int32_t bus_index)
{
    if (!processor || bus_index < 0)
        return 0;

    auto* bus = processor->getBus(true, bus_index);

    if (!bus)
        return 0;

    return bus->getNumberOfChannels();
}

int32_t plugin_output_bus_channels(int32_t bus_index)
{
    if (!processor || bus_index < 0)
        return 0;

    auto* bus = processor->getBus(false, bus_index);

    if (!bus)
        return 0;

    return bus->getNumberOfChannels();
}


// -----------------------------------------------------------------------------
// Persistent plugin state
// -----------------------------------------------------------------------------

uint32_t plugin_state_version()
{
    return 1;
}

int32_t plugin_state_capture()
{
    if (!processor)
        return 0;

    MemoryBlock state;
    processor->getStateInformation(state);

    stateBuffer.resize(state.getSize());

    if (!stateBuffer.empty())
        std::memcpy(
            stateBuffer.data(),
            state.getData(),
            state.getSize());

    return static_cast<int32_t>(stateBuffer.size());
}

int32_t plugin_state_buffer_ptr()
{
    if (stateBuffer.empty())
        return 0;

    return static_cast<int32_t>(
        reinterpret_cast<uintptr_t>(stateBuffer.data()));
}

int32_t plugin_state_prepare_load(int32_t size)
{
    if (size < 0)
        return 0;

    stateBuffer.resize(static_cast<size_t>(size));

    return plugin_state_buffer_ptr();
}

void plugin_state_commit_load(int32_t size)
{
    if (!processor || size < 0)
        return;

    const auto requestedSize = static_cast<size_t>(size);

    if (requestedSize > stateBuffer.size())
        return;

    processor->setStateInformation(
        stateBuffer.data(),
        size);
}

// -----------------------------------------------------------------------------
// Audio buffer
// -----------------------------------------------------------------------------

int32_t plugin_buffer_ptr()
{
    return static_cast<int32_t>(
        reinterpret_cast<uintptr_t>(
            wasmBuffer.data()));
}


// -----------------------------------------------------------------------------
// Audio processing
// -----------------------------------------------------------------------------

// ABI v2: interleaved input buses followed by interleaved output buses.
// Each frame contains all channels in JUCE bus order.
void plugin_process_buses(int32_t ptr, int32_t sample_count)
{
    if (!initialized || !processor || ptr != plugin_buffer_ptr()
        || sample_count < 0 || sample_count > currentBlockSize)
        return;

    const int inChannels = processor->getTotalNumInputChannels();
    const int outChannels = processor->getTotalNumOutputChannels();
    const uint64_t required = static_cast<uint64_t>(sample_count)
        * static_cast<uint64_t>(inChannels + outChannels);
    if (required > MAX_BUFFER_SAMPLES)
        return;

    const int bufferChannels = juce::jmax(inChannels, outChannels);
    audioBuffer.setSize(bufferChannels, sample_count, false, false, true);
    audioBuffer.clear();

    for (int sample = 0; sample < sample_count; ++sample)
        for (int channel = 0; channel < inChannels; ++channel)
            audioBuffer.setSample(channel, sample,
                wasmBuffer[sample * inChannels + channel]);

    midiBuffer.clear();
    processor->processBlock(audioBuffer, midiBuffer);

    const size_t outputOffset = static_cast<size_t>(sample_count) * inChannels;
    for (int sample = 0; sample < sample_count; ++sample)
        for (int channel = 0; channel < outChannels; ++channel)
            wasmBuffer[outputOffset + static_cast<size_t>(sample) * outChannels + channel]
                = audioBuffer.getSample(channel, sample);
}

void plugin_process(
    int32_t ptr,
    int32_t sample_count)
{
    if (!initialized || !processor)
        return;

    if (ptr != plugin_buffer_ptr())
        return;

    if (sample_count < 0)
        return;

    if (currentChannels <= 0)
        return;

    const auto requiredSamples =
        static_cast<uint64_t>(sample_count) *
        static_cast<uint64_t>(currentChannels);

    if (requiredSamples > MAX_BUFFER_SAMPLES)
        return;

    // JUCE uses a single buffer containing all input buses, including
    // sidechain channels. The host ABI currently supplies only the main bus.
    const auto juceChannels = juce::jmax(
        processor->getTotalNumInputChannels(),
        processor->getTotalNumOutputChannels());
    const auto bufferChannels = juce::jmax(currentChannels, juceChannels);

    // Resize the JUCE buffer to the current block size.
    audioBuffer.setSize(
        bufferChannels,
        sample_count,
        false,
        false,
        true);

    // -------------------------------------------------------------------------
    // Interleaved ABI buffer -> JUCE AudioBuffer
    // -------------------------------------------------------------------------

    for (int sample = 0; sample < sample_count; ++sample)
    {
        for (int channel = 0;
             channel < currentChannels;
             ++channel)
        {
            const auto index =
                sample * currentChannels + channel;

            audioBuffer.setSample(
                channel,
                sample,
                wasmBuffer[index]);
        }
    }

    // Additional input buses are unconnected for now. Clear their channels
    // explicitly, including on subsequent blocks when buffers are reused.
    for (int channel = currentChannels; channel < bufferChannels; ++channel)
        audioBuffer.clear(channel, 0, sample_count);

    midiBuffer.clear();

    // -------------------------------------------------------------------------
    // Actual JUCE DSP
    // -------------------------------------------------------------------------

    processor->processBlock(
        audioBuffer,
        midiBuffer);

    // -------------------------------------------------------------------------
    // JUCE AudioBuffer -> interleaved ABI buffer
    // -------------------------------------------------------------------------

    for (int sample = 0; sample < sample_count; ++sample)
    {
        for (int channel = 0;
             channel < currentChannels;
             ++channel)
        {
            const auto index =
                sample * currentChannels + channel;

            wasmBuffer[index] =
                audioBuffer.getSample(
                    channel,
                    sample);
        }
    }
}

// -----------------------------------------------------------------------------
// Plugin metadata
// -----------------------------------------------------------------------------

int32_t plugin_name(
    int32_t bufferPtr,
    int32_t bufferSize)
{
    if (!processor)
        return 0;

    return copyString(
        processor->getName(),
        bufferPtr,
        bufferSize);
}

// -----------------------------------------------------------------------------
// Parameter enumeration
// -----------------------------------------------------------------------------

int32_t plugin_parameter_count()
{
    if (!processor)
        return 0;

    return processor->getParameters().size();
}


int32_t plugin_parameter_id(
    int32_t index,
    int32_t bufferPtr,
    int32_t bufferSize)
{
    auto* parameter = getParameter(index);

    if (!parameter)
        return 0;

    auto* parameterWithID =
        dynamic_cast<AudioProcessorParameterWithID*>(parameter);

    if (!parameterWithID)
        return 0;

    return copyString(
        parameterWithID->paramID,
        bufferPtr,
        bufferSize);
}


int32_t plugin_parameter_name(
    int32_t index,
    int32_t bufferPtr,
    int32_t bufferSize)
{
    auto* parameter = getParameter(index);

    if (!parameter)
        return 0;

    return copyString(
        parameter->getName(256),
        bufferPtr,
        bufferSize);
}


int32_t plugin_parameter_label(
    int32_t index,
    int32_t bufferPtr,
    int32_t bufferSize)
{
    auto* parameter = getParameter(index);

    if (!parameter)
        return 0;

    return copyString(
        parameter->getLabel(),
        bufferPtr,
        bufferSize);
}


// -----------------------------------------------------------------------------
// Parameter range
// -----------------------------------------------------------------------------

float plugin_parameter_min(int32_t index)
{
    auto* parameter = getRangedParameter(index);

    if (!parameter)
        return 0.0f;

    const auto& range =
        parameter->getNormalisableRange();

    return range.start;
}


float plugin_parameter_max(int32_t index)
{
    auto* parameter = getRangedParameter(index);

    if (!parameter)
        return 0.0f;

    const auto& range =
        parameter->getNormalisableRange();

    return range.end;
}


float plugin_parameter_default(int32_t index)
{
    auto* parameter = getParameter(index);

    if (!parameter)
        return 0.0f;

    auto* ranged =
        dynamic_cast<RangedAudioParameter*>(parameter);

    if (ranged)
    {
        return ranged->convertFrom0to1(
            parameter->getDefaultValue());
    }

    return parameter->getDefaultValue();
}


float plugin_parameter_step(int32_t index)
{
    auto* parameter = getRangedParameter(index);

    if (!parameter)
        return 0.0f;

    const auto& range =
        parameter->getNormalisableRange();

    return range.interval;
}


int32_t plugin_parameter_flags(int32_t index)
{
    return getParameterFlags(index);
}


// Return the UTF-8 label of a JUCE AudioParameterChoice item.
// This is part of the optional Choice ABI and is called by the host when
// inspecting the plugin's parameter metadata (not from the audio callback).
int32_t plugin_parameter_choice(
    int32_t parameterIndex,
    int32_t choiceIndex,
    int32_t bufferPtr,
    int32_t bufferSize)
{
    auto* choice = dynamic_cast<AudioParameterChoice*>(
        getParameter(parameterIndex));

    if (choice == nullptr
        || choiceIndex < 0
        || choiceIndex >= choice->choices.size())
        return 0;

    return copyString(
        choice->choices[choiceIndex],
        bufferPtr,
        bufferSize);
}


// -----------------------------------------------------------------------------
// Parameter values
// -----------------------------------------------------------------------------

float plugin_parameter_get(int32_t index)
{
    auto* parameter = getParameter(index);

    if (!parameter)
        return 0.0f;

    return parameter->getValue();
}


void plugin_parameter_set(
    int32_t index,
    float normalizedValue)
{
    auto* parameter = getParameter(index);

    if (!parameter)
        return;

    normalizedValue =
        std::clamp(
            normalizedValue,
            0.0f,
            1.0f);

    parameter->setValueNotifyingHost(
        normalizedValue);
}

}