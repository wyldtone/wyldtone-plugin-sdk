// A minimal stereo effect. The default is unity gain.
declare name "Faust Stereo Gain";
gain = hslider("Gain [unit:dB]", 0, -24, 12, 0.1);
linearGain = pow(10, gain / 20);
process = *(linearGain), *(linearGain);
