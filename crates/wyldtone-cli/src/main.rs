mod cpp;
mod juce;
mod faust;
mod dsp_test;
mod benchmark;
mod developer;
mod init_juce;

use std::{env, error::Error, fs, path::{Path,PathBuf}, process::{Command,Stdio}};
use wyldtone_plugin_api::{config::PluginConfig, manifest::PluginManifest, categories::{normalize_category,category_error}};
use wyldtone_plugin_runtime::wasm_abi::WasmPlugin;
use wyldtone_plugin_packager::package::{create_wap_with_icon,create_wap_with_icon_and_notices,package_name};
use wyldtone_plugin_runtime::dsp_probe::ProbeProfile;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn dependency(name: &str) -> Result<()> {
    let ok = Command::new(name).arg("--version")
        .stdout(Stdio::null()).stderr(Stdio::null()).status()
        .map(|status| status.success()).unwrap_or(false);
    if ok { Ok(()) } else { Err(format!("Required build tool not found: {name}").into()) }
}

fn read_config(path: &Path) -> Result<PluginConfig> {
    let mut config: PluginConfig = serde_json::from_slice(&fs::read(path)?)?;
    if let Some(category) = config.category.as_deref() {
        config.category = Some(normalize_category(category).ok_or_else(|| category_error(category))?.to_owned());
    }
    if config.id.trim().is_empty() || config.name.trim().is_empty() {
        return Err("Plugin id and name must not be empty".into());
    }
    if let Some(audio) = &config.audio { audio.validate()?; }
    if let Some(license) = &config.license { license.validate()?; }
    Ok(config)
}

/// Fail fast on missing/inaccessible license notices *before* invoking
/// CMake/JUCE, DSP probes and WAP packaging. The ZIP entry name from
/// manifest.plugin.license.notices is relative to plugin.config.json.
fn license_notice_files(config: &PluginConfig, plugin_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let Some(license) = &config.license else { return Ok(Vec::new()); };
    // Path::parent("plugin.config.json") can return the empty path when
    // building from the project root: resolve that as the working directory.
    let project_root = if plugin_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        plugin_dir
    };
    let project_root = fs::canonicalize(project_root).map_err(|err| format!(
        "Cannot resolve plugin project directory '{}': {err}", project_root.display()
    ))?;
    let mut files = Vec::with_capacity(license.notices.len());
    for notice in &license.notices {
        let file = project_root.join(&notice.path);
        let resolved = fs::canonicalize(&file).map_err(|err| format!(
            "Missing or inaccessible WAP license notice '{}': expected '{}' \
             (relative to plugin.config.json). Create that file or correct license.notices[].path. \
             Files linked from outside the mounted project directory must be copied inside. ({err})",
            notice.component, file.display()
        ))?;
        // Never package files outside the plugin root through symlinks.
        if !resolved.starts_with(&project_root) {
            return Err(format!(
                "WAP license notice '{}' resolves outside the plugin directory: '{}'",
                notice.path, resolved.display()
            ).into());
        }
        if !resolved.is_file() {
            return Err(format!(
                "WAP license notice '{}' is not a regular file: '{}'",
                notice.path, resolved.display()
            ).into());
        }
        let contents = fs::read(&resolved).map_err(|err| format!(
            "Cannot read WAP license notice '{}': '{}' ({err})",
            notice.path, resolved.display()
        ))?;
        if contents.is_empty() || contents.len() > 64 * 1024
            || std::str::from_utf8(&contents).is_err()
        {
            return Err(format!(
                "WAP license notice '{}' must contain 1..=65536 bytes of UTF-8 text: '{}'",
                notice.path, resolved.display()
            ).into());
        }
        files.push((notice.path.clone(), resolved));
    }
    Ok(files)
}

fn icon_for(config: &PluginConfig, dir: &Path) -> Result<Option<PathBuf>> {
    if let Some(icon) = &config.icon {
        let relative = Path::new(icon);
        if relative.is_absolute() || relative.components().any(|p| matches!(p, std::path::Component::ParentDir | std::path::Component::Prefix(_))) {
            return Err("Icon path must be relative to the plugin directory".into());
        }
        let extension = relative.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["png", "svg"].contains(&extension) { return Err("Icon must be SVG or PNG".into()); }
        let selected = dir.join(relative);
        if !selected.is_file() {
            return Err(format!(
                "Configured plugin icon '{}' is missing or is not a file: '{}'",
                icon, selected.display()
            ).into());
        }
        return Ok(Some(selected));
    }
    for file in ["icon.svg", "icon.png"] {
        let candidate = dir.join(file);
        if candidate.is_file() { return Ok(Some(candidate)); }
    }
    Ok(None)
}

fn generate_manifest(wasm: &Path, config: &PluginConfig, output: &Path) -> Result<()> {
    let engine = WasmPlugin::create_engine()?;
    let mut plugin = WasmPlugin::from_file(&engine, wasm)?;
    let parameters = plugin.load_parameters()?;
    // Validate the actual WASM exports, not merely the JSON declaration.
    match (config.audio.as_ref(), plugin.bus_layout_channels()?) {
        (Some(expected), Some((inputs, outputs))) => {
            let want_inputs = expected.inputs.iter().map(|&x| x as usize).collect::<Vec<_>>();
            let want_outputs = expected.outputs.iter().map(|&x| x as usize).collect::<Vec<_>>();
            if inputs != want_inputs || outputs != want_outputs {
                return Err(format!("WASM bus layout {:?} -> {:?} differs from plugin.config.json {:?} -> {:?}", inputs, outputs, want_inputs, want_outputs).into());
            }
            println!("Audio buses validated: {:?} -> {:?}", inputs, outputs);
        }
        (Some(_), None) => return Err("WASM missing four audio bus exports declared in config".into()),
        (None, None) => eprintln!("Warning: no audio buses in config/WASM; legacy host fallback applies"),
        (None, Some((inputs, outputs))) => println!("Audio buses provided by plugin: {:?} -> {:?}", inputs, outputs),
    }
    let manifest = PluginManifest::from_config_and_parameters(config, &parameters);
    manifest.validate_against(&parameters)?;
    fs::write(output, serde_json::to_vec_pretty(&manifest)?)?;
    println!("Manifest: {} parameters -> {}", parameters.len(), output.display());
    Ok(())
}

// The default build runs the offline DSP parameter probe. Inactive parameters
// are advisory by default; CI may opt into a strict failure policy.
#[derive(Debug, PartialEq, Eq)]
struct BuildOptions {
    skip_dsp_test: bool,
    fail_on_inactive: bool,
    dsp_profile: ProbeProfile,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self { skip_dsp_test: false, fail_on_inactive: false, dsp_profile: ProbeProfile::Balanced }
    }
}

fn parse_build_options(flags: &[String]) -> Result<BuildOptions> {
    let mut options = BuildOptions::default();
    let mut seen_profile = false;
    let mut i = 0;
    while i < flags.len() {
        match flags[i].as_str() {
            "--skip-dsp-test" if !options.skip_dsp_test => options.skip_dsp_test = true,
            "--fail-on-inactive" if !options.fail_on_inactive => options.fail_on_inactive = true,
            "--dsp-profile" if !seen_profile => {
                i += 1;
                let value = flags.get(i).ok_or("Missing value for --dsp-profile")?;
                options.dsp_profile = ProbeProfile::parse(value)?;
                seen_profile = true;
            }
            other => return Err(format!("Unknown or duplicate build option: {other}").into()),
        }
        i += 1;
    }
    if options.skip_dsp_test && options.fail_on_inactive {
        return Err("--skip-dsp-test and --fail-on-inactive cannot be combined".into());
    }
    Ok(options)
}

fn build(path: &Path, options: BuildOptions) -> Result<()> {
    let config = read_config(path)?;
    let dir = path.parent().ok_or("Invalid config path")?;
    let selected_backends = usize::from(config.cpp.is_some())
        + usize::from(config.juce.is_some())
        + usize::from(config.faust.is_some());
    if selected_backends != 1 {
        return Err("Select exactly one backend (cpp, juce or faust)".into());
    }
    // Do this before the potentially expensive JUCE compilation and DSP
    // probe. The packager verifies these files again at archive creation.
    let notices = license_notice_files(&config, dir)?;
    let icon = icon_for(&config, dir)?;
    dependency("em++")?;
    let output_dir = dir.join("build").join("wyldtone");
    fs::create_dir_all(&output_dir)?;
    let wasm = output_dir.join("plugin.wasm");
    if let Some(cpp_cfg) = &config.cpp {
        cpp::build(cpp_cfg, dir, &wasm, config.audio.as_ref())?;
    } else if let Some(juce_cfg) = &config.juce {
        juce::build(juce_cfg, &config.name, dir, &wasm)?;
    } else if let Some(faust_cfg) = &config.faust {
        dependency("faust")?;
        faust::build(faust_cfg, &config.name, dir, &wasm, config.audio.as_ref())?;
    }
    if !wasm.is_file() { return Err(format!("Expected WASM output not found: {}", wasm.display()).into()); }
    let manifest = output_dir.join("manifest.json");
    generate_manifest(&wasm, &config, &manifest)?;
    if options.skip_dsp_test {
        println!("DSP parameter test: skipped (--skip-dsp-test)");
    } else {
        println!("Running DSP parameter test...");
        // The probe writes dsp-test-report.json next to plugin.wasm. A parameter
        // without an observable change is only a warning by default. Actual
        // WASM execution failures (e.g. a trap or non-finite samples) abort the
        // build; --fail-on-inactive also makes parameter warnings fatal for CI.
        dsp_test::run(&wasm, options.fail_on_inactive, options.dsp_profile)?;
    }
    let dest = output_dir.join(package_name(&config.name));
    create_wap_with_icon_and_notices(&wasm, &manifest, &dest, icon.as_deref(), &notices)?;
    println!("WAP: {}", dest.display());
    Ok(())
}

fn help() {
    println!(r#"Wyldtone Plugin SDK CLI
  wyldtone new <directory> --template cpp|juce|faust [--manufacturer NAME]
  wyldtone doctor
  wyldtone init-juce <project-dir> --id ID --name NAME --description TEXT --category CATEGORY [--processor CLASS]
  wyldtone build [directory|plugin.config.json] [--dsp-profile quick|balanced|deep] [--skip-dsp-test | --fail-on-inactive]
  wyldtone inspect <plugin.wap|plugin.wasm>
  wyldtone validate <plugin.wap>
  wyldtone manifest <plugin.wasm> <plugin.config.json> <output.json>
  wyldtone package <plugin.wasm> <manifest.json> <output.wap>
  wyldtone test-dsp <plugin.wasm> [--profile quick|balanced|deep] [--fail-on-inactive]
  wyldtone benchmark <plugin.wasm> [--reference-id ID] [--scenario NAME] [--samples N] [--param id=0..1]... [--report path]"#);
}

fn build_target(args: &[String]) -> Result<(PathBuf, &[String])> {
    let (candidate, flags) = match args.split_first() {
        Some((first, remaining)) if !first.starts_with('-') => (PathBuf::from(first), remaining),
        _ => (PathBuf::from("plugin.config.json"), args),
    };
    let config = if candidate.is_dir() { candidate.join("plugin.config.json") } else { candidate };
    if !config.is_file() {return Err(format!("Plugin configuration not found: {}",config.display()).into());}
    Ok((config,flags))
}

fn parse_test_options(flags: &[String]) -> Result<(ProbeProfile, bool)> {
    let mut profile = ProbeProfile::Balanced;
    let mut strict = false;
    let mut seen_profile = false;
    let mut i = 0;
    while i < flags.len() {
        match flags[i].as_str() {
            "--fail-on-inactive" if !strict => strict = true,
            "--profile" if !seen_profile => {
                i += 1;
                profile = ProbeProfile::parse(flags.get(i).ok_or("Missing value for --profile")?)?;
                seen_profile = true;
            }
            other => return Err(format!("Unknown or duplicate test-dsp option: {other}").into()),
        }
        i += 1;
    }
    Ok((profile, strict))
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, rest @ ..] if cmd == "build" => {
            let (config,flags) = build_target(rest)?;
            build(&config, parse_build_options(flags)?)
        },
        [cmd, rest @ ..] if cmd == "new" => developer::new(rest),
        [cmd, rest @ ..] if cmd == "init-juce" => init_juce::run(rest),
        [cmd] if cmd == "doctor" => developer::doctor(),
        [cmd, path] if cmd == "inspect" => developer::inspect(Path::new(path)),
        [cmd, path] if cmd == "validate" => developer::validate(Path::new(path)),
        [cmd, wasm, config, output] if cmd == "manifest" => generate_manifest(Path::new(wasm), &read_config(Path::new(config))?, Path::new(output)),
        [cmd, wasm, manifest, output] if cmd == "package" => create_wap_with_icon(Path::new(wasm), Path::new(manifest), Path::new(output), None),
        [cmd, wasm, flags @ ..] if cmd == "test-dsp" => {
            let (profile, strict) = parse_test_options(flags)?;
            dsp_test::run(Path::new(wasm), strict, profile)
        },
        [cmd, wasm, flags @ ..] if cmd == "benchmark" => {
            benchmark::run(Path::new(wasm), benchmark::parse(flags)?)
        },
        [cmd] if cmd == "help" || cmd == "--help" || cmd == "-h" => {help();Ok(())},
        [] => {help();Ok(())},
        _ => {help(); Err("Invalid CLI arguments".into()) }
    }
}

fn main() {
    if let Err(err) = run() { eprintln!("Error: {err}"); std::process::exit(1); }
}

#[cfg(test)]
mod build_option_tests {
    use super::*;

    #[test]
    fn dsp_probe_runs_by_default() {
        assert_eq!(parse_build_options(&[]).unwrap(), BuildOptions::default());
    }

    #[test]
    fn supports_advisory_opt_out_and_strict_ci_mode() {
        assert_eq!(
            parse_build_options(&["--skip-dsp-test".to_string()]).unwrap(),
            BuildOptions { skip_dsp_test: true, fail_on_inactive: false, dsp_profile: ProbeProfile::Balanced }
        );
        assert_eq!(
            parse_build_options(&["--fail-on-inactive".to_string()]).unwrap(),
            BuildOptions { skip_dsp_test: false, fail_on_inactive: true, dsp_profile: ProbeProfile::Balanced }
        );
    }

    #[test]
    fn refuses_conflicting_duplicate_or_unknown_options() {
        for flags in [
            vec!["--skip-dsp-test", "--fail-on-inactive"],
            vec!["--skip-dsp-test", "--skip-dsp-test"],
            vec!["--unknown"],
        ] {
            let values = flags.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
            assert!(parse_build_options(&values).is_err());
        }
    }

    #[test]
    fn license_notice_preflight_fails_early_and_names_exact_project_path() {
        let project = std::env::temp_dir().join(format!(
            "wyldtone-license-preflight-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&project).unwrap();
        let config: PluginConfig = serde_json::from_value(serde_json::json!({
            "id": "com.example.licensed",
            "name": "Licensed effect",
            "manufacturer": "Example",
            "version": "1.0.0",
            "license": {
                "expression": "MIT",
                "notices": [{
                    "component": "Upstream DSP",
                    "spdx": "MIT",
                    "path": "licenses/MIT.txt"
                }]
            }
        })).unwrap();

        let expected = project.join("licenses/MIT.txt");
        let error = license_notice_files(&config, &project).unwrap_err().to_string();
        assert!(error.contains("Missing or inaccessible WAP license notice"));
        assert!(error.contains(&expected.display().to_string()));
        assert!(error.contains("plugin.config.json"));
        assert!(!project.join("build").exists(), "preflight must not start compilation");

        fs::create_dir_all(expected.parent().unwrap()).unwrap();
        fs::write(&expected, b"MIT License\nCopyright Example\n").unwrap();
        let sources = license_notice_files(&config, &project).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].0, "licenses/MIT.txt");
        assert_eq!(sources[0].1, fs::canonicalize(&expected).unwrap());

        fs::write(&expected, b"\xff\xfe").unwrap();
        let error = license_notice_files(&config, &project).unwrap_err().to_string();
        assert!(error.contains("UTF-8 text"));
        assert!(error.contains("licenses/MIT.txt"));

        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn license_notice_preflight_is_optional_for_unlicensed_plugins() {
        let config: PluginConfig = serde_json::from_value(serde_json::json!({
            "id": "com.example.unlicensed",
            "name": "No notices",
            "manufacturer": "Example",
            "version": "1.0.0"
        })).unwrap();
        assert!(license_notice_files(&config, Path::new("")).unwrap().is_empty());
    }

    #[test]
    fn parses_dsp_profiles_for_build_and_standalone_tests() {
        let opts = parse_build_options(&["--dsp-profile".into(), "deep".into()]).unwrap();
        assert_eq!(opts.dsp_profile, ProbeProfile::Deep);
        assert!(parse_build_options(&["--dsp-profile".into()]).is_err());
        let (profile, strict) = parse_test_options(&[
            "--profile".into(), "quick".into(), "--fail-on-inactive".into()
        ]).unwrap();
        assert_eq!(profile, ProbeProfile::Quick);
        assert!(strict);
        assert!(parse_test_options(&["--profile".into(), "INVALID".into()]).is_err());
    }

}
