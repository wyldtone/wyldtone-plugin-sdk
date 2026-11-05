// SPDX-License-Identifier: Apache-2.0
// Wyldtone SDK example: bass drive with a clean low-frequency branch.
// A Linkwitz-Riley 4th-order crossover creates phase-matched low/high bands.
// Only the upper band is overdriven; the low band remains clean.
// This is intentionally a basic waveshaper, not a circuit model.

declare name "Wyldtone Bass Mid Drive";
declare description "Bass overdrive with clean lows and a variable high-band blend";
declare version "0.1.0";

fi = library("filters.lib");
si = library("signals.lib");
ma = library("maths.lib");

splitHz = hslider("Crossover [unit:Hz]", 170.0, 70.0, 500.0, 1.0) : si.smoo;
driveDb = hslider("Drive [unit:dB]", 16.0, 0.0, 32.0, 0.5) : si.smoo;
toneHz = hslider("Tone Cutoff [unit:Hz]", 4000.0, 800.0, 12000.0, 10.0) : si.smoo;
blend = hslider("High Band Drive Blend", 0.65, 0.0, 1.0, 0.01) : si.smoo;
outputDb = hslider("Output Gain [unit:dB]", -3.0, -18.0, 12.0, 0.5) : si.smoo;

preGain = pow(10.0, driveDb / 20.0);
postGain = pow(10.0, outputDb / 20.0);

// The upper band keeps its clean component when Blend is 0.
// The low band never enters the waveshaper.
shaper = *(preGain) : ma.tanh : fi.lowpass(2, min(toneHz, 0.45 * ma.SR));
upperBand = _ <: *(1.0 - blend), (shaper : *(blend)) :> _;

// crossover2LR4 produces two phase-aligned outputs: low, high.
channel = fi.crossover2LR4(splitHz) : _, upperBand :> *(postGain);

process = channel, channel;
