// SPDX-License-Identifier: Apache-2.0
// Wyldtone SDK example: low-complexity stereo slapback with one repeat.
// This is deliberately NOT a feedback or tape-saturation delay.

declare name "Wyldtone Stereo Slapback";
declare description "Single-tap stereo delay with tone and adjustable stereo spread";
declare version "0.1.0";

si = library("signals.lib");
de = library("delays.lib");
fi = library("filters.lib");
ma = library("maths.lib");

timeMs = hslider("Delay Time [unit:ms]", 115.0, 45.0, 350.0, 1.0) : si.smoo;
spread = hslider("Stereo Spread", 0.15, 0.0, 0.3, 0.01) : si.smoo;
toneHz = hslider("Tone [unit:Hz]", 5500.0, 900.0, 12000.0, 20.0) : si.smoo;
mix = hslider("Mix", 0.3, 0.0, 0.7, 0.01) : si.smoo;

// At 384 kHz the right-channel maximum is 350*1.3 ms ~= 174,720
// samples; comfortably below our delay line capacity (262,144).
slap(multiplier) = de.sdelay(262144, 1024, timeMs * multiplier * ma.SR / 1000.0)
                 : fi.lowpass(2, toneHz);
voice(multiplier) = _ <: *(1.0 - mix), (slap(multiplier) : *(mix)) :> _;

process = voice(1.0), voice(1.0 + spread);
