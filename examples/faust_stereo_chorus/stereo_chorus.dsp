// SPDX-License-Identifier: Apache-2.0
// Wyldtone SDK example: true stereo modulated fractional-delay chorus.
// Two independent delay lines use phase-offset LFOs for stereo width.

declare name "Wyldtone Stereo Chorus";
declare description "Stereo chorus with rate, delay, depth, width and wet mix";
declare version "0.1.0";

si = library("signals.lib");
de = library("delays.lib");
os = library("oscillators.lib");
ma = library("maths.lib");

rateHz = hslider("Rate [unit:Hz]", 0.7, 0.1, 5.0, 0.05) : si.smoo;
baseMs = hslider("Base Delay [unit:ms]", 12.0, 6.0, 22.0, 0.5) : si.smoo;
depthMs = hslider("Depth [unit:ms]", 8.0, 0.0, 12.0, 0.5) : si.smoo;
width = hslider("Stereo Width", 0.85, 0.0, 1.0, 0.01) : si.smoo;
mix = hslider("Mix", 0.4, 0.0, 0.7, 0.01) : si.smoo;

// At 192 kHz max delay is (22+12) ms = 6528 samples, below 16384.
// The LFO drives delay time smoothly (fractional interpolation).
delaySamples(phase) = (baseMs + depthMs * (0.5 + 0.5 * os.oscp(rateHz, phase))) * ma.SR / 1000.0;
wet(phase) = de.fdelay(16384, delaySamples(phase));
voice(phase) = _ <: *(1.0 - mix), (wet(phase) : *(mix)) :> _;

// Right LFO phase moves by up to 180 degrees relative to left.
process = voice(0.0), voice(width * ma.PI);
