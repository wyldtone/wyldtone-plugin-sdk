// SPDX-License-Identifier: Apache-2.0
// Original example processor (no JUCE framework source is redistributed).
#pragma once
#include "JuceHeader.h"

class WyldtoneJuceGainProcessor final : public juce::AudioProcessor
{
public:
    WyldtoneJuceGainProcessor();
    const juce::String getName() const override { return "JUCE Gain Example"; }
    void prepareToPlay(double, int) override {}
    void releaseResources() override {}
    bool isBusesLayoutSupported(const BusesLayout& layouts) const override;
    void processBlock(juce::AudioBuffer<float>&, juce::MidiBuffer&) override;

    bool acceptsMidi() const override { return false; }
    bool producesMidi() const override { return false; }
    bool isMidiEffect() const override { return false; }
    double getTailLengthSeconds() const override { return 0.0; }
    int getNumPrograms() override { return 1; }
    int getCurrentProgram() override { return 0; }
    void setCurrentProgram(int) override {}
    const juce::String getProgramName(int) override { return {}; }
    void changeProgramName(int, const juce::String&) override {}
    void getStateInformation(juce::MemoryBlock&) override {}
    void setStateInformation(const void*, int) override {}
    bool hasEditor() const override { return false; }
    juce::AudioProcessorEditor* createEditor() override { return nullptr; }
private:
    juce::AudioParameterFloat* gain_ = nullptr;
};
