//! Faust -> generated C++ -> existing Wyldtone WASM ABI and WAP pipeline.
use std::{error::Error, fs, path::{Component, Path, PathBuf}, process::Command};
use wyldtone_plugin_api::config::{AudioBuses, CppConfig, FaustConfig};
use crate::cpp;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn safe_source(plugin_dir: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() || path.components().any(|part| matches!(part, Component::ParentDir | Component::Prefix(_))) {
        return Err("Faust source must be relative and must not contain '..'".into());
    }
    if path.extension().and_then(|e| e.to_str()) != Some("dsp") {
        return Err("Faust source must have the .dsp extension".into());
    }
    let full = plugin_dir.join(path);
    if !full.is_file() { return Err(format!("Faust source not found: {}", full.display()).into()); }
    Ok(full)
}

pub fn build(config: &FaustConfig, name: &str, plugin_dir: &Path,
             output: &Path, audio: Option<&AudioBuses>) -> Result<()> {
    let layout = audio.ok_or("Faust requires an explicit 'audio' bus layout in plugin.config.json")?;
    layout.validate()?;
    if layout.inputs.len() != 1 || layout.outputs.len() != 1 {
        return Err("Faust currently supports exactly one main input and one output bus".into());
    }
    if config.extra_inputs > 4 || layout.inputs[0] + config.extra_inputs > 8 {
        return Err("Faust extra_inputs must be 0..=4 and total DSP inputs <= 8".into());
    }
    if (config.extra_inputs == 0) != config.extra_sources.is_empty() {
        return Err("Faust extra_inputs and extra_sources must be configured together".into());
    }
    // Reject traversal and absolute paths. C++ sources are compiled from the
    // plugin directory, not from arbitrary paths outside the workspace.
    for extra in &config.extra_sources {
        let relative = Path::new(extra);
        if relative.is_absolute() || relative.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
            || relative.extension().and_then(|e| e.to_str()) != Some("cpp")
            || !plugin_dir.join(relative).is_file() {
            return Err(format!("Invalid Faust extra source: {extra}").into());
        }
    }
    let source = safe_source(plugin_dir, &config.source)?;
    let arch = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/faust/wyldtone_arch.cpp");
    let bridge = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/faust/wyldtone_bridge.cpp");
    if !arch.is_file() || !bridge.is_file() {
        return Err("Wyldtone Faust adapter files are missing".into());
    }
    let out_dir = output.parent().ok_or("Invalid Faust build output path")?;
    fs::create_dir_all(out_dir)?;
    let generated = out_dir.join("faust_generated.cpp");
    let wrapper = out_dir.join("faust_wrapper.cpp");

    println!("Generate Faust C++: {}", source.display());
    let status = Command::new("faust")
        .arg("-lang").arg("cpp")
        .arg("-i")
        .arg("-cn").arg("WyldFaustDSP")
        .arg("-a").arg(&arch)
        .arg("-o").arg(&generated)
        .arg(&source)
        .status()
        .map_err(|e| format!("Failed to run Faust compiler: {e}. Install Faust in the SDK image"))?;
    if !status.success() { return Err("Faust C++ code generation failed".into()); }
    if !generated.is_file() { return Err("Faust did not produce faust_generated.cpp".into()); }

    // A name is a compile-time string literal, not C++ code.
    let quoted_name = serde_json::to_string(name)?;
    let mut content = format!(
        "#define WYLDTONE_FAUST_PLUGIN_NAME {quoted_name}\n#define WYLDTONE_FAUST_EXTRA_INPUTS {}\n#define WYLDTONE_FAUST_MAIN_INPUTS {}\n",
        config.extra_inputs, layout.inputs[0]
    );
    content.push_str(&fs::read_to_string(&bridge)?);
    fs::write(&wrapper, content)?;
    let mut sources = vec!["build/wyldtone/faust_wrapper.cpp".into()];
    sources.extend(config.extra_sources.iter().cloned());
    let cpp = CppConfig { sources };
    // The existing C++ builder handles all WASM exports, bus validation and
    // the same Rust manifest/packager used for non-Faust plugins.
    cpp::build(&cpp, plugin_dir, output, Some(layout))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_paths_outside_the_plugin() {
        assert!(safe_source(Path::new("."), "../other.dsp").is_err());
        assert!(safe_source(Path::new("."), "/tmp/foo.dsp").is_err());
        assert!(safe_source(Path::new("."), "bad.cpp").is_err());
    }
}
