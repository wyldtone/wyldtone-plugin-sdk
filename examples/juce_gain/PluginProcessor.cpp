// SPDX-License-Identifier: Apache-2.0
#include "PluginProcessor.h"

WyldtoneJuceGainProcessor::WyldtoneJuceGainProcessor()
    : juce::AudioProcessor(juce::AudioProcessor::BusesProperties()
        .withInput("Input", juce::AudioChannelSet::stereo(), true)
        .withOutput("Output", juce::AudioChannelSet::stereo(), true))
{
    auto* gain = new juce::AudioParameterFloat("gain", "Gain",
        juce::NormalisableRange<float>(0.0f, 2.0f, 0.01f), 1.0f);
    addParameter(gain);
    gain_ = gain;
}

bool WyldtoneJuceGainProcessor::isBusesLayoutSupported(const BusesLayout& layouts) const
{
    return layouts.getMainInputChannelSet() == juce::AudioChannelSet::stereo()
        && layouts.getMainOutputChannelSet() == juce::AudioChannelSet::stereo();
}

void WyldtoneJuceGainProcessor::processBlock(juce::AudioBuffer<float>& audio,
                                              juce::MidiBuffer&)
{
    const float level = gain_ != nullptr ? gain_->get() : 1.0f;
    for (int channel = 0; channel < audio.getNumChannels(); ++channel)
        audio.applyGain(channel, 0, audio.getNumSamples(), level);
}
