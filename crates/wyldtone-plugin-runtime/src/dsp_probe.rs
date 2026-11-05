//! Offline parameter-to-audio smoke tests for standalone Wyldtone WASM plugins.
//!
//! This is not a proof that a parameter works correctly. It answers a narrower
//! question: do two parameter values produce observably different audio when
//! everything else (input and initial plugin state) is held constant?

use std::{collections::HashSet, error::Error, path::Path};
use wyldtone_plugin_api::parameter::{Parameter, ParameterKind};
use wasmtime::Module;
use crate::wasm_abi::WasmPlugin;

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 256;
const BLOCKS: usize = 64;
// Contextual runs are shorter to keep the default build affordable on a
// two-core CI/build host; the baseline smoke test retains 64 blocks.
const CONTEXT_BLOCKS: usize = 24;
const BALANCED_PER_PARAMETER: usize = 12;
const BALANCED_TOTAL: usize = 48;
const DEEP_PER_PARAMETER: usize = 42;
const DEEP_TOTAL: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeProfile { Quick, Balanced, Deep }

impl ProbeProfile {
    pub fn name(self) -> &'static str {
        match self { Self::Quick => "quick", Self::Balanced => "balanced", Self::Deep => "deep" }
    }
    pub fn parse(s: &str) -> ProbeResult<Self> {
        match s {
            "quick" => Ok(Self::Quick),
            "balanced" => Ok(Self::Balanced),
            "deep" => Ok(Self::Deep),
            _ => Err(format!("Unknown DSP profile {s:?}; expected quick, balanced, or deep").into()),
        }
    }
    fn limits(self) -> (usize, usize) {
        match self {
            Self::Quick => (0, 0),
            Self::Balanced => (BALANCED_PER_PARAMETER, BALANCED_TOTAL),
            Self::Deep => (DEEP_PER_PARAMETER, DEEP_TOTAL),
        }
    }
}

#[derive(Clone, Debug)]
struct Context {
    assignments: Vec<(usize, f32)>,
    description: String,
}
impl Context {
    fn defaults() -> Self { Self { assignments: Vec::new(), description: "defaults".into() } }
}

const MAX_BUFFER_FLOATS: usize = 16_384;
const ABSOLUTE_THRESHOLD: f64 = 0.000_02;
const RELATIVE_THRESHOLD: f64 = 0.002;

pub type ProbeResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct ParameterProbe {
    pub id: String,
    pub name: String,
    pub verdict: &'static str,
    pub best_signal: &'static str,
    pub absolute_rms_difference: f64,
    pub relative_rms_difference: f64,
    pub low_readback: f32,
    pub high_readback: f32,
    pub best_context: String,
    pub tested_contexts: usize,
}

#[derive(Debug)]
pub struct ProbeReport {
    pub plugin: String,
    pub sample_rate: u32,
    pub block_size: usize,
    pub channels: usize,
    pub parameters: Vec<ParameterProbe>,
    pub skipped_reason: Option<String>,
    pub profile: &'static str,
    pub contextual_cases: usize,
    pub context_budget_exhausted: bool,
}

impl ProbeReport {
    pub fn inactive_count(&self) -> usize {
        self.parameters.iter().filter(|p| p.verdict != "changed").count()
    }
}

#[derive(Clone, Copy, Debug)]
enum Signal {
    LowSine,
    MidSine,
    HighSine,
    QuietSine,
    TwoTone,
    Noise,
}

impl Signal {
    fn name(self) -> &'static str {
        match self {
            Self::LowSine => "sine_110hz_loud",
            Self::MidSine => "sine_440hz_loud",
            Self::HighSine => "sine_2000hz_loud",
            Self::QuietSine => "sine_440hz_quiet",
            Self::TwoTone => "two_tone",
            Self::Noise => "seeded_noise",
        }
    }

    fn sample(self, frame: usize, channel: usize) -> f32 {
        let t = frame as f64 / SAMPLE_RATE as f64;
        let pi2 = std::f64::consts::PI * 2.0;
        let gain = if channel % 2 == 0 { 1.0 } else { 0.89 };
        (gain * match self {
            Self::LowSine => 0.4 * (pi2 * 110.0 * t).sin(),
            Self::MidSine => 0.4 * (pi2 * 440.0 * t).sin(),
            Self::HighSine => 0.4 * (pi2 * 2000.0 * t).sin(),
            Self::QuietSine => 0.012 * (pi2 * 440.0 * t).sin(),
            Self::TwoTone => {
                0.24 * (pi2 * 93.0 * t).sin() + 0.24 * (pi2 * 790.0 * t).sin()
            }
            Self::Noise => {
                // Deterministic pseudo-random signal, independent of run order.
                let mut x = (frame as u32).wrapping_add(1).wrapping_mul(0x9e37_79b9);
                x ^= x >> 16;
                x = x.wrapping_mul(0x7feb_352d);
                x ^= x >> 15;
                x = x.wrapping_mul(0x846c_a68b);
                x ^= x >> 16;
                0.3 * (2.0 * (x as f64 / u32::MAX as f64) - 1.0)
            }
        }) as f32
    }
}

const SIGNALS: [Signal; 6] = [
    Signal::LowSine,
    Signal::MidSine,
    Signal::HighSine,
    Signal::QuietSine,
    Signal::TwoTone,
    Signal::Noise,
];

// The plugin ABI exposes one shared float buffer. Use the same convention as
// the audio host: for process_buses, interleaved input followed by output;
// otherwise the legacy in-place process() call.
fn checked_process(plugin: &mut WasmPlugin, input: &[f32], frames: usize) -> ProbeResult<Vec<f32>> {
    let count = input.len();
    let bus_abi = plugin.plugin_process_buses.is_some();
    let total_floats = if bus_abi { count.checked_mul(2) } else { Some(count) }
        .ok_or("DSP buffer size overflow")?;
    if total_floats > MAX_BUFFER_FLOATS {
        return Err("Audio block exceeds Wyldtone ABI buffer size".into());
    }
    let offset = usize::try_from(plugin.buffer_ptr).map_err(|_| "Invalid WASM audio buffer pointer")?;
    let bytes = total_floats.checked_mul(4).ok_or("DSP buffer size overflow")?;
    let end = offset.checked_add(bytes).ok_or("WASM memory offset overflow")?;
    let available = plugin.memory.data_size(&plugin.store);
    if end > available {
        let pages = (end - available).div_ceil(65_536);
        plugin.memory.grow(&mut plugin.store, pages as u64)?;
    }
    {
        let mem = plugin.memory.data_mut(&mut plugin.store);
        for (i, sample) in input.iter().enumerate() {
            mem[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&sample.to_le_bytes());
        }
        if bus_abi {
            mem[offset + count * 4..end].fill(0);
        }
    }
    if let Some(f) = plugin.plugin_process_buses.clone() {
        f.call(&mut plugin.store, (plugin.buffer_ptr, frames as i32))?;
    } else {
        plugin.plugin_process.call(&mut plugin.store, (plugin.buffer_ptr, count as i32))?;
    }
    let read_offset = if bus_abi { offset + count * 4 } else { offset };
    let mem = plugin.memory.data(&plugin.store);
    let mut output = Vec::with_capacity(count);
    for chunk in mem[read_offset..read_offset + count * 4].chunks_exact(4) {
        let value = f32::from_le_bytes(chunk.try_into()?);
        if !value.is_finite() { return Err("Plugin returned NaN or infinite audio sample".into()); }
        output.push(value);
    }
    Ok(output)
}

fn render(
    engine: &wasmtime::Engine,
    module: &Module,
    channels: usize,
    parameter_index: usize,
    normalized: f32,
    signal: Signal,
    context: &Context,
    blocks: usize,
) -> ProbeResult<(Vec<f32>, f32)> {
    // Fresh instance per rendering prevents a previous parameter value,
    // envelope or effect tail from contaminating the comparison.
    let mut plugin = WasmPlugin::from_module(engine, module)?;
    let mut params = plugin.load_parameters()?;
    plugin.prepare(SAMPLE_RATE, channels, FRAMES)?;
    plugin.initialize_parameter_defaults(&mut params);
    // Both compared renderings must receive identical context settings.
    for &(index, value) in &context.assignments {
        plugin.plugin_parameter_set.call(&mut plugin.store, (index as i32, value))?;
    }
    plugin.plugin_parameter_set.call(&mut plugin.store, (parameter_index as i32, normalized))?;
    let readback = plugin.plugin_parameter_get.call(&mut plugin.store, (parameter_index as i32,))?;
    if !readback.is_finite() { return Err("Plugin returned a non-finite parameter value".into()); }
    let mut output = Vec::with_capacity(blocks * FRAMES * channels);
    for block in 0..blocks {
        let mut input = Vec::with_capacity(FRAMES * channels);
        for frame in 0..FRAMES {
            for channel in 0..channels {
                input.push(signal.sample(block * FRAMES + frame, channel));
            }
        }
        output.extend(checked_process(&mut plugin, &input, FRAMES)?);
    }
    Ok((output, readback))
}

#[derive(Debug, Clone, Copy)]
struct Difference {
    absolute: f64,
    relative: f64,
}

fn difference(a: &[f32], b: &[f32]) -> Difference {
    assert!(!a.is_empty() && a.len() == b.len());
    let mut energy_a = 0.0;
    let mut energy_b = 0.0;
    let mut delta = 0.0;
    for (&left, &right) in a.iter().zip(b) {
        let l = left as f64;
        let r = right as f64;
        energy_a += l * l;
        energy_b += r * r;
        delta += (l - r) * (l - r);
    }
    let n = a.len() as f64;
    let absolute = (delta / n).sqrt();
    let reference = (energy_a.max(energy_b) / n).sqrt();
    Difference { absolute, relative: absolute / reference.max(1e-6) }
}

fn observable(d: Difference) -> bool {
    d.absolute > ABSOLUTE_THRESHOLD && d.relative > RELATIVE_THRESHOLD
}

// One-factor contexts cover (target value, one other parameter value).
// Sort discrete choices/switches first so e.g. "Revision = V2" is tested
// before exploring numerous continuous tone controls.
fn context_controls(params: &[Parameter], target: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..params.len()).filter(|&i| i != target).collect();
    indices.sort_by_key(|&i| match &params[i].kind {
        ParameterKind::Choice { .. } => 0,
        ParameterKind::Bool => 1,
        ParameterKind::Int { .. } => 2,
        ParameterKind::Float { .. } => 3,
    });
    indices
}

fn values(param: &Parameter) -> Vec<f32> {
    match &param.kind {
        ParameterKind::Bool => vec![0.0, 1.0],
        ParameterKind::Choice { choices } if choices.len() > 1 && choices.len() <= 5 =>
            (0..choices.len()).map(|i| i as f32 / (choices.len()-1) as f32).collect(),
        _ => vec![0.0, 0.5, 1.0],
    }
}

fn context_value_name(param: &Parameter, normalized: f32) -> String {
    use wyldtone_plugin_api::parameter::ParameterValue;
    match param.value_from_normalized(normalized) {
        ParameterValue::Choice(i) => match &param.kind {
            ParameterKind::Choice { choices } => choices.get(i as usize).cloned()
                .unwrap_or_else(|| format!("choice#{i}")),
            _ => format!("choice#{i}"),
        },
        ParameterValue::Bool(v) => if v { "on".into() } else { "off".into() },
        ParameterValue::Int(i) => format!("{i}"),
        ParameterValue::Float(v) => format!("{v:.3}"),
    }
}

fn push_context(
    cases: &mut Vec<Context>,
    seen: &mut HashSet<String>,
    params: &[Parameter],
    assignments: Vec<(usize, f32)>,
    limit: usize,
) {
    if cases.len() >= limit { return; }
    let mut key = assignments.iter()
        .map(|(i, v)| format!("{i}:{v:.6}"))
        .collect::<Vec<_>>();
    key.sort();
    if seen.insert(key.join("|")) {
        let description = assignments.iter().map(|&(i, v)| {
            format!("{}={} (normalized {v:.3})", params[i].id, context_value_name(&params[i], v))
        }).collect::<Vec<_>>().join(", ");
        cases.push(Context { assignments, description });
    }
}

// Generate at most `limit` contexts instead of constructing the full
// Cartesian product. The parameter under test is intentionally *not* included
// in each context: its endpoints are tested with all other settings fixed.
fn candidate_contexts(
    params: &[Parameter], target: usize, profile: ProbeProfile, limit: usize,
) -> Vec<Context> {
    if profile == ProbeProfile::Quick || limit == 0 { return Vec::new(); }
    let controls = context_controls(params, target);
    let mut cases = Vec::new();
    let mut seen = HashSet::new();
    'single: for &other in &controls {
        for level in values(&params[other]) {
            if (level - params[other].default_normalized_value()).abs() > 1e-5 {
                push_context(&mut cases, &mut seen, params, vec![(other, level)], limit);
                if cases.len() >= limit { break 'single; }
            }
        }
    }
    if profile == ProbeProfile::Deep && cases.len() < limit {
        // Bounded two-control exploration, not an exhaustive covering array.
        'pair: for (position, &left) in controls.iter().enumerate() {
            for &right in controls.iter().skip(position + 1) {
                for l in values(&params[left]) {
                    for r in values(&params[right]) {
                        if (l - params[left].default_normalized_value()).abs() < 1e-5 &&
                           (r - params[right].default_normalized_value()).abs() < 1e-5 { continue; }
                        push_context(&mut cases, &mut seen, params,
                                     vec![(left, l), (right, r)], limit);
                        if cases.len() >= limit { break 'pair; }
                    }
                }
            }
        }
    }
    cases
}

// A single context is compared in isolation with the tested parameter at
// normalized 0 vs 1. We never compare different contexts to one another.
fn compare_context(
    engine: &wasmtime::Engine, module: &Module, channels: usize,
    index: usize, context: &Context, signals: &[Signal], blocks: usize,
) -> ProbeResult<(Difference, &'static str, Signal, f32, f32, bool)> {
    let mut best = Difference { absolute: 0.0, relative: 0.0 };
    let mut best_name = "none";
    let mut best_case = signals[0];
    let mut best_observable = false;
    let mut readbacks = (0.0, 0.0);
    let mut changed = false;
    for &signal in signals {
        let (low, lv) = render(engine, module, channels, index, 0.0, signal, context, blocks)?;
        let (high, hv) = render(engine, module, channels, index, 1.0, signal, context, blocks)?;
        readbacks = (lv, hv);
        let d = difference(&low, &high);
        let this_observable = observable(d);
        if (this_observable && !best_observable) ||
           (this_observable == best_observable && d.relative > best.relative) ||
           (best_name == "none" && d.absolute > 0.0) {
            best = d;
            best_name = signal.name();
            best_case = signal;
            best_observable = this_observable;
        }
        changed |= this_observable;
    }
    Ok((best, best_name, best_case, readbacks.0, readbacks.1, changed))
}

pub fn run(path: &Path) -> ProbeResult<ProbeReport> {
    run_with_profile(path, ProbeProfile::Quick)
}

pub fn run_with_profile(path: &Path, profile: ProbeProfile) -> ProbeResult<ProbeReport> {
    let engine = WasmPlugin::create_engine()?;
    let module = Module::from_file(&engine, path)?;
    let mut plugin = WasmPlugin::from_module(&engine, &module)?;
    let name = plugin.name()?;
    let parameters = plugin.load_parameters()?;
    if parameters.len() > 256 { return Err("More than 256 parameters; test refused".into()); }
    let (inputs, outputs) = plugin.bus_layout_channels()?
        .unwrap_or_else(|| (vec![1], vec![1]));
    let mut report = ProbeReport {
        plugin: name,
        sample_rate: SAMPLE_RATE,
        block_size: FRAMES,
        channels: inputs.first().copied().unwrap_or(0),
        parameters: Vec::new(),
        skipped_reason: None,
        profile: profile.name(),
        contextual_cases: 0,
        context_budget_exhausted: false,
    };
    if parameters.is_empty() {
        report.skipped_reason = Some("Plugin exposes no parameters".to_string());
        return Ok(report);
    }
    if inputs.len() != 1 || outputs.len() != 1 || inputs != outputs || !(1..=2).contains(&report.channels) {
        report.skipped_reason = Some(format!(
            "Currently supports one matching mono or stereo input/output bus; got {inputs:?} -> {outputs:?}"
        ));
        return Ok(report);
    }
    let (max_per_param, max_total) = profile.limits();
    // Avoid rebuilding every contextual experiment for parameters which
    // already demonstrably affect audio in the default configuration.
    for (index, param) in parameters.iter().enumerate() {
        let defaults = Context::defaults();
        let (mut best, mut signal_name, mut best_case, mut low_readback, mut high_readback, mut changed) =
            compare_context(&engine, &module, report.channels, index, &defaults, &SIGNALS, BLOCKS)?;
        let mut best_context = defaults.description.clone();
        let mut tested_contexts = 1;
        if !changed && max_per_param > 0 {
            let remaining = max_total.saturating_sub(report.contextual_cases);
            let candidate_limit = max_per_param.min(remaining);
            let candidates = candidate_contexts(&parameters, index, profile, candidate_limit.saturating_add(1));
            let context_signals = [Signal::Noise, Signal::QuietSine, Signal::TwoTone];
            for context in candidates.iter().take(candidate_limit) {
                report.contextual_cases += 1;
                tested_contexts += 1;
                let (diff, sname, scase, lv, hv, responds) =
                    compare_context(&engine, &module, report.channels, index, context,
                                    &context_signals, CONTEXT_BLOCKS)?;
                if responds || diff.relative > best.relative {
                    best = diff;
                    signal_name = sname;
                    best_case = scase;
                    best_context = context.description.clone();
                    low_readback = lv;
                    high_readback = hv;
                }
                if responds { changed = true; break; }
            }
            if !changed && candidates.len() > candidate_limit {
                report.context_budget_exhausted = true;
            }
        }
        // Signal drift between identical instances can cause false positives.
        // Repeat the winning signal *with the winning context*.
        let unstable = if changed {
            let context = if best_context == "defaults" { defaults } else {
                candidate_contexts(&parameters, index, profile, max_per_param)
                    .into_iter().find(|c| c.description == best_context)
                    .ok_or("Probe context unexpectedly missing")?
            };
            let blocks = if context.assignments.is_empty() { BLOCKS } else { CONTEXT_BLOCKS };
            let (a, _) = render(&engine, &module, report.channels, index, 0.0, best_case, &context, blocks)?;
            let (b, _) = render(&engine, &module, report.channels, index, 0.0, best_case, &context, blocks)?;
            let drift = difference(&a, &b);
            observable(drift) && drift.absolute > best.absolute * 0.1
        } else { false };
        report.parameters.push(ParameterProbe {
            id: param.id.clone(),
            name: param.name.clone(),
            verdict: if (low_readback-high_readback).abs() < 1e-4 {
                "parameter_not_updated"
            } else if unstable { "inconclusive_nondeterministic" }
            else if changed { "changed" } else { "no_observable_change" },
            best_signal: signal_name,
            absolute_rms_difference: best.absolute,
            relative_rms_difference: best.relative,
            low_readback,
            high_readback,
            best_context,
            tested_contexts,
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mock(id: &str, kind: ParameterKind) -> Parameter {
        use wyldtone_plugin_api::parameter::ParameterValue;
        let default = match &kind {
            ParameterKind::Float { .. } => ParameterValue::Float(0.5),
            ParameterKind::Int { .. } => ParameterValue::Int(1),
            ParameterKind::Bool => ParameterValue::Bool(false),
            ParameterKind::Choice { .. } => ParameterValue::Choice(0),
        };
        Parameter { id: id.into(), name: id.into(), label: String::new(), kind,
            default: default.clone(), value: default, automatable: true }
    }
    #[test]
    fn profiles_parse_and_are_bounded() {
        assert_eq!(ProbeProfile::parse("deep").unwrap(), ProbeProfile::Deep);
        assert!(ProbeProfile::parse("no-such-mode").is_err());
        assert!(ProbeProfile::Deep.limits().0 < 100);
    }
    #[test]
    fn contexts_prioritize_choices_and_include_revision_v2() {
        let p = vec![
            mock("mid", ParameterKind::Float { min: 0.0, max: 1.0, step: None }),
            mock("drive", ParameterKind::Float { min: 0.0, max: 1.0, step: None }),
            mock("revision", ParameterKind::Choice { choices: vec!["V1".into(), "V1 late".into(), "V2".into()] }),
        ];
        let cases = candidate_contexts(&p, 0, ProbeProfile::Balanced, 20);
        assert!(cases[0].description.starts_with("revision="));
        assert!(cases.iter().any(|c| c.description.contains("revision=V2")));
        let deep = candidate_contexts(&p, 0, ProbeProfile::Deep, 20);
        assert!(deep.iter().any(|c| c.assignments.len() == 2));
    }
    #[test]
    fn context_enumeration_respects_budget() {
        let mut params = vec![mock("target", ParameterKind::Float { min: 0.0, max: 1.0, step: None })];
        for i in 0..80 {
            params.push(mock(&format!("control_{i}"), ParameterKind::Float {
                min: 0.0, max: 1.0, step: None
            }));
        }
        assert_eq!(candidate_contexts(&params, 0, ProbeProfile::Deep, 7).len(), 7);
        assert!(candidate_contexts(&params, 0, ProbeProfile::Quick, 7).is_empty());
    }
    #[test]
    fn context_value_description_uses_choice_labels() {
        let p = mock("revision", ParameterKind::Choice {
            choices: vec!["V1 Early".into(), "V1 Late".into(), "V2".into()]
        });
        assert_eq!(context_value_name(&p, 1.0), "V2");
    }
    #[test]
    fn signals_are_deterministic_and_nonzero() {
        for signal in SIGNALS {
            let a: Vec<_> = (0..512).map(|i| signal.sample(i, 0)).collect();
            let b: Vec<_> = (0..512).map(|i| signal.sample(i, 0)).collect();
            assert_eq!(a, b);
            assert!(a.iter().any(|v| *v != 0.0));
        }
    }
    #[test]
    fn equal_audio_is_not_a_change() {
        let signal = vec![0.1_f32, -0.2, 0.3];
        assert!(!observable(difference(&signal, &signal)));
    }
    #[test]
    fn gain_change_is_detected() {
        let a = vec![0.05_f32; 1024];
        let b = vec![0.4_f32; 1024];
        assert!(observable(difference(&a, &b)));
    }
    #[test]
    fn tiny_numerical_noise_is_ignored() {
        let a = vec![0.4_f32; 1024];
        let b = vec![0.400001_f32; 1024];
        assert!(!observable(difference(&a, &b)));
    }
}
