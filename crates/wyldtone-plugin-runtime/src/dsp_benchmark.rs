//! Repeatable offline call-time measurements for Wyldtone WASM plugins.
//! Measurements are host-specific and must not be mistaken for real-time guarantees.
use std::{error::Error, path::Path, time::Instant};
use wasmtime::Module;
use crate::wasm_abi::WasmPlugin;

pub type BenchResult<T> = Result<T, Box<dyn Error>>;
pub const BENCHMARK_PROTOCOL: &str = "wyldtone-dsp-benchmark-v1";
pub const SAMPLE_RATE: u32 = 48_000;
pub const BLOCK_SIZES: [usize; 3] = [128, 256, 512];
const WARMUP_BLOCKS: usize = 48;

#[derive(Clone, Debug)]
pub struct ParameterSetting {
    pub id: String,
    pub normalized: f32,
}

#[derive(Clone, Debug)]
pub struct BenchmarkOptions {
    pub samples: usize,
    pub reference_id: String,
    pub scenario: String,
    pub parameters: Vec<ParameterSetting>,
}

impl Default for BenchmarkOptions {
    fn default() -> Self {
        Self { samples: 240, reference_id: "unregistered-local-host".into(), scenario: "default".into(), parameters: Vec::new() }
    }
}

#[derive(Debug)]
pub struct AppliedParameter {
    pub id: String,
    pub normalized_readback: f32,
    pub displayed_value: String,
}

#[derive(Debug)]
pub struct BlockMeasurement {
    pub frames: usize,
    pub samples: usize,
    pub budget_ms: f64,
    pub mean_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub p95_budget_percent: f64,
}

#[derive(Debug)]
pub struct BenchmarkReport {
    pub plugin: String,
    pub protocol: &'static str,
    pub runtime_version: &'static str,
    pub reference_id: String,
    pub scenario: String,
    pub sample_rate: u32,
    pub channels: usize,
    pub settings: Vec<AppliedParameter>,
    pub results: Vec<BlockMeasurement>,
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    assert!(!sorted.is_empty());
    let rank = ((sorted.len() - 1) as f64 * fraction).ceil() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn input_samples(frames: usize, channels: usize) -> Vec<f32> {
    let mut buffer = Vec::with_capacity(frames * channels);
    for frame in 0..frames {
        let t = frame as f64 / SAMPLE_RATE as f64;
        let x = 0.23 * (std::f64::consts::TAU * 110.0 * t).sin()
            + 0.09 * (std::f64::consts::TAU * 940.0 * t).sin();
        for channel in 0..channels {
            buffer.push((x * if channel % 2 == 0 { 1.0 } else { 0.91 }) as f32);
        }
    }
    buffer
}

fn prepare_buffer(plugin: &mut WasmPlugin, input: &[f32], use_buses: bool) -> BenchResult<()> {
    let count = input.len();
    let floats = if use_buses { count.checked_mul(2) } else { Some(count) }
        .ok_or("Benchmark buffer overflow")?;
    if floats > 16_384 { return Err("Benchmark block exceeds host ABI buffer limit".into()); }
    let start = usize::try_from(plugin.buffer_ptr).map_err(|_| "Invalid audio buffer pointer")?;
    let end = start.checked_add(floats.checked_mul(4).ok_or("Buffer overflow")?)
        .ok_or("Memory offset overflow")?;
    let available = plugin.memory.data_size(&plugin.store);
    if end > available {
        plugin.memory.grow(&mut plugin.store, ((end - available).div_ceil(65_536)) as u64)?;
    }
    let memory = plugin.memory.data_mut(&mut plugin.store);
    for (i, sample) in input.iter().enumerate() {
        memory[start + i * 4..start + i * 4 + 4].copy_from_slice(&sample.to_le_bytes());
    }
    if use_buses {
        memory[start + count * 4..end].fill(0);
    }
    Ok(())
}

fn run_one_block(plugin: &mut WasmPlugin, frames: usize, channels: usize, input: &[f32]) -> BenchResult<f64> {
    let use_buses = plugin.plugin_process_buses.is_some();
    prepare_buffer(plugin, input, use_buses)?;
    let begin = Instant::now();
    if let Some(process) = plugin.plugin_process_buses.as_ref() {
        process.call(&mut plugin.store, (plugin.buffer_ptr, frames as i32))?;
    } else {
        plugin.plugin_process.call(&mut plugin.store, (plugin.buffer_ptr, (frames * channels) as i32))?;
    }
    // Only the WASM processing call is timed; input buffer writes are excluded.
    let elapsed_ms = begin.elapsed().as_secs_f64() * 1000.0;
    // Read one output sample to detect gross non-finite failures. The functional
    // DSP probe performs a complete, separately measured output validation.
    let first = if use_buses { input.len() } else { 0 };
    let start = usize::try_from(plugin.buffer_ptr)? + first * 4;
    let memory = plugin.memory.data(&plugin.store);
    let val = f32::from_le_bytes(memory[start..start + 4].try_into()?);
    if !val.is_finite() { return Err("Plugin produced a non-finite audio value".into()); }
    Ok(elapsed_ms)
}

pub fn run(wasm: &Path, options: &BenchmarkOptions) -> BenchResult<BenchmarkReport> {
    if !(32..=2000).contains(&options.samples) {
        return Err("--samples must be between 32 and 2000".into());
    }
    if options.reference_id.is_empty() || options.reference_id.len() > 128 ||
        !options.reference_id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Err("Reference ID must contain 1..128 ASCII letters, digits, '-' '_' or '.'".into());
    }
    if options.scenario.is_empty() || options.scenario.len() > 128 ||
        !options.scenario.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Err("Scenario must contain 1..128 ASCII letters, digits, '-' '_' or '.'".into());
    }
    if options.parameters.len() > 64 { return Err("Too many parameter overrides (max 64)".into()); }
    let engine = WasmPlugin::create_engine()?;
    let module = Module::from_file(&engine, wasm)?;
    let mut plugin_info = WasmPlugin::from_module(&engine, &module)?;
    let plugin_name = plugin_info.name()?;
    let bus_layout = plugin_info.bus_layout_channels()?;
    // A single in/out bus is required for this first benchmark protocol.
    // Sidechain/multi-bus layouts need an explicit, separately specified fixture.
    let channels = match bus_layout {
        Some((input, output)) if input.len() == 1 && output.len() == 1 && input[0] == output[0] && (1..=2).contains(&input[0]) => input[0],
        Some(_) => return Err("Benchmark v1 supports only one matching mono/stereo input and output bus".into()),
        None => 2, // Legacy plugin ABI: benchmark a stereo host configuration.
    };
    let mut report = BenchmarkReport {
        plugin: plugin_name,
        protocol: BENCHMARK_PROTOCOL,
        runtime_version: env!("CARGO_PKG_VERSION"),
        reference_id: options.reference_id.clone(),
        scenario: options.scenario.clone(),
        sample_rate: SAMPLE_RATE,
        channels,
        settings: Vec::new(),
        results: Vec::new(),
    };
    let all_params = plugin_info.load_parameters()?;
    // Avoid allowing duplicate or unknown overrides to silently change the run.
    let mut seen = std::collections::HashSet::new();
    for requested in &options.parameters {
        if !requested.normalized.is_finite() || !(0.0..=1.0).contains(&requested.normalized) {
            return Err(format!("Parameter {} must be normalized 0..1", requested.id).into());
        }
        if !seen.insert(requested.id.as_str()) {
            return Err(format!("Duplicate parameter override: {}", requested.id).into());
        }
        if !all_params.iter().any(|p| p.id == requested.id) {
            return Err(format!("Unknown plugin parameter: {}", requested.id).into());
        }
    }
    drop(plugin_info);

    for &frames in &BLOCK_SIZES {
        let mut plugin = WasmPlugin::from_module(&engine, &module)?;
        let mut params = plugin.load_parameters()?;
        // Some plugins select oversampling and other internal topologies during
        // plugin_init/prepareToPlay. Apply the scenario before AND after prepare.
        plugin.initialize_parameter_defaults(&mut params);
        for requested in &options.parameters {
            let idx = params.iter().position(|p| p.id == requested.id)
                .ok_or_else(|| format!("Unknown parameter {}", requested.id))?;
            plugin.plugin_parameter_set.call(&mut plugin.store, (idx as i32, requested.normalized))?;
        }
        plugin.prepare(SAMPLE_RATE, channels, frames)?;
        let mut applied = Vec::new();
        for requested in &options.parameters {
            let idx = params.iter().position(|p| p.id == requested.id)
                .ok_or_else(|| format!("Unknown parameter {}", requested.id))?;
            plugin.plugin_parameter_set.call(&mut plugin.store, (idx as i32, requested.normalized))?;
            let actual = plugin.plugin_parameter_get.call(&mut plugin.store, (idx as i32,))?;
            if !actual.is_finite() { return Err("Non-finite parameter readback".into()); }
            let displayed_value = match params[idx].value_from_normalized(actual) {
                wyldtone_plugin_api::parameter::ParameterValue::Choice(choice) => {
                    match &params[idx].kind {
                        wyldtone_plugin_api::parameter::ParameterKind::Choice { choices } => {
                            choices.get(choice as usize).cloned().unwrap_or_else(|| format!("choice_{choice}"))
                        }
                        _ => format!("choice_{choice}"),
                    }
                }
                other => format!("{other:?}"),
            };
            applied.push(AppliedParameter {
                id: requested.id.clone(), normalized_readback: actual, displayed_value
            });
        }
        if report.settings.is_empty() { report.settings = applied; }
        let buffer = input_samples(frames, channels);
        for _ in 0..WARMUP_BLOCKS { run_one_block(&mut plugin, frames, channels, &buffer)?; }
        let mut times = Vec::with_capacity(options.samples);
        for _ in 0..options.samples {
            times.push(run_one_block(&mut plugin, frames, channels, &buffer)?);
        }
        let sum: f64 = times.iter().sum();
        times.sort_by(|a, b| a.total_cmp(b));
        let budget = frames as f64 * 1000.0 / SAMPLE_RATE as f64;
        report.results.push(BlockMeasurement {
            frames,
            samples: options.samples,
            budget_ms: budget,
            mean_ms: sum / options.samples as f64,
            median_ms: percentile(&times, 0.5),
            p95_ms: percentile(&times, 0.95),
            max_ms: *times.last().ok_or("No benchmark samples")?,
            p95_budget_percent: percentile(&times, 0.95) / budget * 100.0,
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentile_examples() {
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.5), 3.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.95), 5.0);
    }
    #[test]
    fn input_is_deterministic() { assert_eq!(input_samples(512, 2), input_samples(512, 2)); }
    #[test]
    fn benchmark_defaults_are_bounded() { assert!((32..=2000).contains(&BenchmarkOptions::default().samples)); }
}
