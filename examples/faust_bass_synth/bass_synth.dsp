// SPDX-License-Identifier: Apache-2.0
// Wyldtone Bass Synth: an original MONOPHONIC, bass-input-controlled
// synth effect. Inspired by the general class of 1970s/80s bass synth
// pedals; not an emulation of any manufacturer's algorithm or presets.
// Pitch and gate are supplied by the plugin-local C++ YIN tracker through
// two INTERNAL Faust signal inputs. The Wyldtone host still sees stereo I/O.

declare name "Wyldtone Bass Synth";
declare description "Monophonic tracked bass synth with sub, oscillator shapes, filter envelope and clean blend";
declare version "0.4.0";

an = library("analyzers.lib");
os = library("oscillators.lib");
fi = library("filters.lib");
si = library("signals.lib");
ma = library("maths.lib");
ba = library("basics.lib");

// Wave Shape is a stepped slider rather than a choice widget: the current
// Wyldtone Faust bridge registers sliders but does not expose Faust menu labels.
waveShape = hslider("Wave Shape (0 Tri 1 Saw 2 Square)", 1.0, 0.0, 2.0, 1.0);
wetMix = hslider("Synth Mix", 0.72, 0.0, 1.0, 0.01) : si.smoo;
subMix = hslider("Sub Octave", 0.42, 0.0, 1.0, 0.01) : si.smoo;
cutoffHz = hslider("Filter Cutoff [unit:Hz]", 950, 80, 6000, 10) : si.smoo;
resonance = hslider("Resonance", 0.36, 0.0, 1.0, 0.01) : si.smoo;
filterEnvOct = hslider("Filter Env Amount [unit:oct]", 1.7, -3.0, 4.0, 0.1) : si.smoo;
attackMs = hslider("Attack [unit:ms]", 4, 1, 160, 1) : si.smoo;
releaseMs = hslider("Release [unit:ms]", 160, 30, 800, 5) : si.smoo;
glideMs = hslider("Glide [unit:ms]", 18, 0, 350, 1) : si.smoo;
gateDb = hslider("Gate Threshold [unit:dB]", -44, -60, -16, 1) : si.smoo;
// This parameter is handled by C++ and kept in Faust so the host can adjust
// the tracking source using the normal plugin parameter API.
trackingSource = hslider("Tracking Source (0 Mix 1 Left 2 Right)", 0, 0, 2, 1);
// This control is also forwarded to the C++ YIN sidecar by the Faust bridge.
// A small audible gate-smoothing change keeps the control in Faust's UI tree.
trackingResponseMs = hslider("Tracking Response [unit:ms]", 55, 25, 105, 5);
// A manual aid for diagnosing octave errors, not an automatic octave fix.
octaveShift = hslider("Tracking Octave (-1 Sub 0 Normal 1 Up)", 0, -1, 1, 1);
lfoHz = hslider("LFO Rate [unit:Hz]", 2.6, 0.1, 9, 0.1) : si.smoo;
lfoDepth = hslider("Filter LFO Depth", 0.0, 0, 1, 0.01) : si.smoo;
stereoWidth = hslider("Stereo Width", 0.25, 0.0, 1.0, 0.01) : si.smoo;
outputDb = hslider("Output Gain [unit:dB]", -5, -18, 6, 0.5) : si.smoo;

clamp(x, lo, hi) = min(hi, max(lo, x));

// Triangle and saw oscillators are alias-suppressed; square uses polyBLEP.
// All three are calculated so switching Wave Shape does not change ABI or DSP state.
oscillator(freq) = tri*triWeight + saw*sawWeight + sq*squareWeight
with {
    tri = os.polyblep_triangle(freq);
    saw = os.sawtooth(freq);
    sq = os.polyblep_square(freq);
    triWeight = clamp(1.0 - abs(waveShape - 0.0), 0.0, 1.0);
    sawWeight = clamp(1.0 - abs(waveShape - 1.0), 0.0, 1.0);
    squareWeight = clamp(1.0 - abs(waveShape - 2.0), 0.0, 1.0);
};

process(left, right, yinPitchHz, yinNoteGate) = outputL, outputR
with {
    // Detection runs in the C++ controller. Keep the original stereo dry
    // inputs separate from these two hidden control-rate signal inputs.
    trackMix = clamp(1.0 - abs(trackingSource - 0.0), 0.0, 1.0);
    trackLeft = clamp(1.0 - abs(trackingSource - 1.0), 0.0, 1.0);
    trackRight = clamp(1.0 - abs(trackingSource - 2.0), 0.0, 1.0);
    mono = 0.5 * (left + right) * trackMix + left * trackLeft + right * trackRight;
    // When no reliable note is available, C++ sets pitch to zero and gate off.
    // Glide only smooths new valid notes; it does not lock to the old note after
    // silence (the C++ gate is reset on a new attack).
    pitchHz = clamp(max(30.0, yinPitchHz) * pow(2.0, octaveShift), 15.0, 840.0)
            : si.smooth(ba.tau2pole(max(0.003, glideMs / 1000.0)));
    amplitude = an.amp_follower_ar(max(0.001, attackMs / 1000.0),
                                   max(0.02, releaseMs / 1000.0), mono);
    threshold = pow(10.0, gateDb / 20.0);
    gate = clamp((amplitude - threshold) / max(0.002, threshold * 0.75), 0.0, 1.0)
         * (yinNoteGate > 0.5)
         : si.smooth(ba.tau2pole(clamp(trackingResponseMs * 0.00005, 0.001, 0.006)));
    // Scaled so a normal passive/active bass DI signal opens the synth.
    noteLevel = clamp(amplitude * 5.0, 0.0, 1.0) * gate;
    filterEnvelope = clamp(amplitude * 5.0, 0.0, 1.0);
    modulator = os.osc(lfoHz);
    activeCutoff = clamp(cutoffHz * pow(2.0, filterEnvOct * filterEnvelope
                                     + lfoDepth * modulator * 1.5), 60.0, 7500.0);
    q = 0.65 + 6.0 * resonance;
    // A subtle detune gives width without compromising the common sub voice.
    freqL = pitchHz * (1.0 - 0.003 * stereoWidth);
    freqR = pitchHz * (1.0 + 0.003 * stereoWidth);
    voiceL = (oscillator(freqL) * (1.0 - 0.55 * subMix)
             + os.osc(max(16.0, 0.5 * pitchHz)) * 0.85 * subMix)
            : fi.resonlp(activeCutoff, q, 0.8);
    voiceR = (oscillator(freqR) * (1.0 - 0.55 * subMix)
             + os.osc(max(16.0, 0.5 * pitchHz)) * 0.85 * subMix)
            : fi.resonlp(activeCutoff * (1.0 + stereoWidth * 0.012), q, 0.8);
    makeUp = pow(10.0, outputDb / 20.0);
    synthL = voiceL * noteLevel * makeUp;
    synthR = voiceR * noteLevel * makeUp;
    outputL = left * (1.0 - wetMix) + synthL * wetMix;
    outputR = right * (1.0 - wetMix) + synthR * wetMix;
};
