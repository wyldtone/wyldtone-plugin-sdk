use std::{error::Error, fs, path::Path};
use serde_json::json;
use wyldtone_plugin_runtime::dsp_probe::{self, ProbeProfile};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub fn run(wasm: &Path, fail_on_inactive: bool, profile: ProbeProfile) -> Result<()> {
    let report = dsp_probe::run_with_profile(wasm, profile)?;
    println!(
        "DSP probe [{}]: {} @ {} Hz / {} frames",
        report.profile, report.plugin, report.sample_rate, report.block_size
    );
    if let Some(reason) = &report.skipped_reason {
        println!("SKIP: {reason}");
    }
    let mut items = Vec::new();
    for param in &report.parameters {
        let label = if param.verdict == "changed" { "PASS" } else { "WARN" };
        println!(
            "{label}: {} [{}] - {} (signal: {}, context: {}, RMS diff: {:.4}%, contexts: {})",
            param.name, param.id, param.verdict, param.best_signal,
            param.best_context, param.relative_rms_difference * 100.0,
            param.tested_contexts
        );
        items.push(json!({
            "id": param.id,
            "name": param.name,
            "verdict": param.verdict,
            "best_signal": param.best_signal,
            "best_context": param.best_context,
            "tested_contexts": param.tested_contexts,
            "relative_rms_difference": param.relative_rms_difference,
            "absolute_rms_difference": param.absolute_rms_difference,
            "low_readback": param.low_readback,
            "high_readback": param.high_readback
        }));
    }
    let out = wasm.with_file_name("dsp-test-report.json");
    let doc = json!({
        "plugin": report.plugin,
        "profile": report.profile,
        "sample_rate": report.sample_rate,
        "block_size": report.block_size,
        "channels": report.channels,
        "skipped_reason": report.skipped_reason,
        "contextual_cases": report.contextual_cases,
        "context_budget_exhausted": report.context_budget_exhausted,
        "parameters": items
    });
    fs::write(&out, serde_json::to_vec_pretty(&doc)?)?;
    println!("Report: {}", out.display());
    if report.inactive_count() > 0 {
        println!("WARN guidance: docs/dsp-tests.md (interpretation and troubleshooting)");
    }
    if report.context_budget_exhausted {
        println!("WARN: DSP context-test budget reached; remaining interactions are untested");
    }
    println!(
        "{} parameters with no verified change out of {}",
        report.inactive_count(), report.parameters.len()
    );
    if fail_on_inactive {
        if report.skipped_reason.is_some() {
            return Err("DSP test was skipped; cannot pass --fail-on-inactive".into());
        }
        if report.inactive_count() > 0 {
            return Err("Some parameters showed no verifiable effect in tested contexts".into());
        }
    }
    Ok(())
}
