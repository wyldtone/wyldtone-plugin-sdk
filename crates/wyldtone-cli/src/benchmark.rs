use std::{error::Error, fs, path::{Path, PathBuf}};
use wyldtone_plugin_runtime::dsp_benchmark::{self, BenchmarkOptions, ParameterSetting};
use serde_json::json;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct CliOptions {
    pub benchmark: BenchmarkOptions,
    pub output: Option<PathBuf>,
}

pub fn parse(flags: &[String]) -> Result<CliOptions> {
    let mut benchmark = BenchmarkOptions::default();
    let mut output = None;
    let mut seen_samples = false;
    let mut seen_scenario = false;
    let mut seen_reference = false;
    let mut i = 0;
    while i < flags.len() {
        match flags[i].as_str() {
            "--samples" if !seen_samples => {
                i += 1;
                benchmark.samples = flags.get(i).ok_or("Missing --samples value")?.parse()?;
                seen_samples = true;
            }
            "--reference-id" if !seen_reference => {
                i += 1;
                benchmark.reference_id = flags.get(i).ok_or("Missing --reference-id value")?.clone();
                seen_reference = true;
            }
            "--scenario" if !seen_scenario => {
                i += 1;
                benchmark.scenario = flags.get(i).ok_or("Missing --scenario value")?.clone();
                seen_scenario = true;
            }
            "--param" => {
                i += 1;
                let value = flags.get(i).ok_or("Missing --param value")?;
                let (id, normalized) = value.split_once('=').ok_or("--param requires id=normalized (0..1)")?;
                if id.is_empty() { return Err("Parameter ID must not be empty".into()); }
                benchmark.parameters.push(ParameterSetting { id: id.into(), normalized: normalized.parse()? });
            }
            "--report" if output.is_none() => {
                i += 1;
                output = Some(PathBuf::from(flags.get(i).ok_or("Missing --report path")?));
            }
            unknown => return Err(format!("Unknown or duplicate benchmark option: {unknown}").into()),
        }
        i += 1;
    }
    Ok(CliOptions { benchmark, output })
}

pub fn run(wasm: &Path, options: CliOptions) -> Result<()> {
    let report = dsp_benchmark::run(wasm, &options.benchmark)?;
    let measurements: Vec<_> = report.results.iter().map(|m| {
        json!({
            "block_frames": m.frames,
            "sample_count": m.samples,
            "audio_budget_ms": m.budget_ms,
            "mean_ms": m.mean_ms,
            "median_ms": m.median_ms,
            "p95_ms": m.p95_ms,
            "max_ms": m.max_ms,
            "p95_budget_percent": m.p95_budget_percent
        })
    }).collect();
    let settings: Vec<_> = report.settings.iter().map(|p| json!({
        "id": p.id,
        "normalized_readback": p.normalized_readback,
        "displayed_value": p.displayed_value
    })).collect();
    // A benchmark never claims universal CPU utilization or real-time safety.
    let data = json!({
        "schema_version": 1,
        "benchmark_protocol": report.protocol,
        "runtime_crate_version": report.runtime_version,
        "reference_id": report.reference_id,
        "scenario": report.scenario,
        "plugin": report.plugin,
        "environment": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "measurement": "WASM process-call wall clock only; input transfer excluded",
        "sample_rate_hz": report.sample_rate,
        "channels": report.channels,
        "applied_parameter_values": settings,
        "results": measurements,
        "comparison_scope": "Compare only identical reference_id, protocol, runtime/toolchain, settings and server load conditions"
    });
    println!("DSP benchmark: {} / {} / {} Hz", report.plugin, report.reference_id, report.sample_rate);
    for m in &report.results {
        println!("  {:>3} frames: median {:.4} ms, p95 {:.4} ms, budget {:.1}% (p95)",
            m.frames, m.median_ms, m.p95_ms, m.p95_budget_percent);
    }
    let path = options.output.unwrap_or_else(|| wasm.with_file_name("dsp-benchmark-report.json"));
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    fs::write(&path, serde_json::to_vec_pretty(&data)?)?;
    println!("Report: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_and_errors() {
        let v = ["--samples", "150", "--reference-id", "wyserver-v1", "--param", "drive=0.5"]
            .map(str::to_string);
        let result = parse(&v).unwrap();
        assert_eq!(result.benchmark.samples, 150);
        assert_eq!(result.benchmark.reference_id, "wyserver-v1");
        assert_eq!(result.benchmark.scenario, "default");
        assert_eq!(result.benchmark.parameters[0].id, "drive");
        assert!(parse(&["--param".into(), "invalid".into()]).is_err());
        assert!(parse(&["--samples".into()]).is_err());
    }
}
