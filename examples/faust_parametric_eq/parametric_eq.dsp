// SPDX-License-Identifier: Apache-2.0
// Wyldtone SDK example: stereo four-band parametric equalizer.
// Low / high shelves and two fully parametric mid bands.
// This example demonstrates Faust's filters.lib and parameter smoothing.

declare name "Wyldtone Parametric EQ";
declare description "Four-band stereo EQ: two shelves and two peaking bands";
declare version "0.1.0";

fi = library("filters.lib");
si = library("signals.lib");
ma = library("maths.lib");

lowGain = hslider("Low Gain [unit:dB]", 0.0, -15.0, 15.0, 0.5) : si.smoo;
lowFreq = hslider("Low Frequency [unit:Hz]", 100.0, 25.0, 400.0, 1.0) : si.smoo;

mid1Gain = hslider("Low Mid Gain [unit:dB]", 0.0, -15.0, 15.0, 0.5) : si.smoo;
mid1Freq = hslider("Low Mid Frequency [unit:Hz]", 350.0, 80.0, 2000.0, 1.0) : si.smoo;
mid1Q = hslider("Low Mid Q", 1.0, 0.3, 8.0, 0.05) : si.smoo;

mid2Gain = hslider("High Mid Gain [unit:dB]", 0.0, -15.0, 15.0, 0.5) : si.smoo;
mid2Freq = hslider("High Mid Frequency [unit:Hz]", 1800.0, 400.0, 8000.0, 1.0) : si.smoo;
mid2Q = hslider("High Mid Q", 1.0, 0.3, 8.0, 0.05) : si.smoo;

highGain = hslider("High Gain [unit:dB]", 0.0, -15.0, 15.0, 0.5) : si.smoo;
highFreq = hslider("High Frequency [unit:Hz]", 5000.0, 1500.0, 12000.0, 1.0) : si.smoo;

outputDb = hslider("Output Gain [unit:dB]", 0.0, -18.0, 12.0, 0.5) : si.smoo;
outputLinear = pow(10.0, outputDb / 20.0);

// Guard frequency controls for host sample rates below 44.1 kHz.
safeHz(freq) = max(20.0, min(freq, 0.45 * ma.SR));

channel = fi.low_shelf(lowGain, safeHz(lowFreq))
       : fi.peak_eq_cq(mid1Gain, safeHz(mid1Freq), mid1Q)
       : fi.peak_eq_cq(mid2Gain, safeHz(mid2Freq), mid2Q)
       : fi.high_shelf(highGain, safeHz(highFreq))
       : *(outputLinear);

// Two independent processing states, identical shared controls.
process = channel, channel;
