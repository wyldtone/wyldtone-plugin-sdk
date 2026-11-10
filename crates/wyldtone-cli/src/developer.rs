//! Small developer-facing commands. No dependencies on the proprietary engine.
use std::{collections::HashSet, env, error::Error, fs, path::{Path, PathBuf}, process::{Command, Stdio}};

use wyldtone_plugin_api::{manifest::{PluginManifest, MANIFEST_FORMAT_VERSION}, parameter::Parameter, categories::{normalize_category,category_error}};
use wyldtone_plugin_packager::package::{load_wap, load_wap_icon};
use wyldtone_plugin_runtime::wasm_abi::WasmPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Template { Cpp, Juce, Faust }

impl Template {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "cpp" => Ok(Self::Cpp),
            "juce" => Ok(Self::Juce),
            "faust" => Ok(Self::Faust),
            _ => Err(format!("Unknown template '{value}' (choose cpp, juce or faust)").into()),
        }
    }
    fn label(self) -> &'static str {
        match self { Self::Cpp => "Vanilla C++", Self::Juce => "JUCE", Self::Faust => "Faust" }
    }
}

fn project_name(path: &Path) -> Result<String> {
    let name = path.file_name().and_then(|s| s.to_str())
        .ok_or("Provide a project directory such as plugins/MyEffect")?;
    // Restrict to a portable project stem, including valid characters for a C++ string.
    if name.len() > 64 || !name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Project name must start with a letter and contain only ASCII letters, digits, '-' or '_' (max 64 characters)".into());
    }
    Ok(name.to_owned())
}

#[derive(Debug)]
struct NewOptions { directory: PathBuf, template: Template, manufacturer: String }

fn parse_new(args: &[String]) -> Result<NewOptions> {
    let (first, flags) = args.split_first().ok_or("Usage: wyldtone new <directory> --template cpp|juce|faust [--manufacturer NAME]")?;
    if first.starts_with('-') { return Err("Project directory must be the first argument".into()); }
    let directory = PathBuf::from(first);
    project_name(&directory)?;
    let mut template = None;
    let mut manufacturer = "Your Name".to_owned();
    let mut seen_manufacturer = false;
    let mut i = 0;
    while i < flags.len() {
        let flag = &flags[i];
        let value = flags.get(i + 1).ok_or_else(||format!("Missing value for {flag}"))?;
        match flag.as_str() {
            "--template" if template.is_none() => template = Some(Template::parse(value)?),
            "--manufacturer" if !seen_manufacturer => {
                if value.trim().is_empty() { return Err("Manufacturer must not be empty".into()); }
                manufacturer = value.clone();
                seen_manufacturer = true;
            }
            _ => return Err(format!("Unknown or duplicate new option: {flag}").into()),
        }
        i += 2;
    }
    Ok(NewOptions {directory, template:template.ok_or("--template cpp|juce|faust is required")?, manufacturer})
}

pub fn new(args: &[String]) -> Result<()> {
    let options = parse_new(args)?;
    let name = project_name(&options.directory)?;
    if options.directory.exists() {
        return Err(format!("Refusing to overwrite existing path: {}", options.directory.display()).into());
    }
    // Requiring the parent to exist avoids accidentally creating projects outside
    // the workspace because of a misspelled path.
    let parent = options.directory.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if !parent.is_dir() {return Err(format!("Parent directory does not exist: {}", parent.display()).into());}
    let id = format!("org.example.{}", name.to_ascii_lowercase().replace('_', "-"));
    let (backend_key, backend, files): (&str, serde_json::Value, Vec<(&str, String)>) = match options.template {
        Template::Cpp => {
            let source = include_str!("../../../examples/gain/plugin.cpp")
                .replace("SDK Gain Example", &name);
            ("cpp", serde_json::json!({"sources":["plugin.cpp"]}), vec![("plugin.cpp", format!("// SPDX-License-Identifier: Apache-2.0\n{source}"))])
        }
        Template::Juce => {
            let header = include_str!("../../../examples/juce_gain/PluginProcessor.h")
                .replace("JUCE Gain Example", &name);
            let processor = include_str!("../../../examples/juce_gain/PluginProcessor.cpp");
            ("juce", serde_json::json!({"processor":"WyldtoneJuceGainProcessor", "header":"PluginProcessor.h", "modules":[], "include_dirs":[], "simd128":false}),
                vec![("PluginProcessor.h", header.into()), ("PluginProcessor.cpp", processor.into())])
        }
        Template::Faust => {
            let source = include_str!("../../../examples/faust_gain/gain.dsp")
                .replace("Faust Stereo Gain", &name);
            ("faust", serde_json::json!({"source":"main.dsp"}), vec![("main.dsp", format!("// SPDX-License-Identifier: Apache-2.0\n{source}"))])
        }
    };
    let mut config = serde_json::json!({
        "id":id, "name":name.clone(), "manufacturer": options.manufacturer,
        "version":"0.1.0", "category":"Utility",
        "audio":{"inputs":[2], "outputs":[2]}
    });
    config[backend_key] = backend;
    fs::create_dir(&options.directory)?;
    let result = (|| -> Result<()> {
        fs::write(options.directory.join("plugin.config.json"), serde_json::to_vec_pretty(&config)?)?;
        for (filename, content) in files {fs::write(options.directory.join(filename), content)?;}
        fs::write(options.directory.join(".gitignore"), "build/\n")?;
        fs::write(options.directory.join("README.md"), format!(
            "# {name}\n\nGenerated {template} starter plugin.\n\nEdit `plugin.config.json` before publication: choose your own globally unique `id`, manufacturer, category and version.\n\nBuild from the SDK workspace with:\n\n```sh\ndocker compose run --rm sdk build {path}/plugin.config.json\n```\n\nJUCE is not bundled or relicensed. See `docs/juce-licensing.md` in the SDK before distributing JUCE-derived binaries.\n",
            template=options.template.label(), path=options.directory.display()
        ))?;
        Ok(())
    })();
    if result.is_err() {let _=fs::remove_dir_all(&options.directory);}
    result?;
    println!("Created {} project: {}",options.template.label(), options.directory.display());
    println!("Next: wyldtone build {}/plugin.config.json",options.directory.display());
    Ok(())
}

fn in_path(name: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {return false};
    for dir in env::split_paths(&paths) {
        for candidate in [name.to_owned(),format!("{name}.exe"),format!("{name}.cmd"),format!("{name}.bat")] {
            if dir.join(candidate).is_file() {return true;}
        }
    }
    false
}

fn tool(name: &str, flag: &str, required: bool) -> bool {
    let result = Command::new(name).arg(flag).stdout(Stdio::piped()).stderr(Stdio::piped()).output();
    let label = if required {"REQUIRED"} else {"OPTIONAL"};
    match result {
        Ok(out) if out.status.success() => {
            let output = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let version = output.lines().chain(stderr.lines()).find(|s|!s.trim().is_empty()).unwrap_or("installed");
            println!("[OK] {name:<10} {version}");
            true
        }
        _ => {
            println!("[{}] {name}: not available ({label})",if required {"FAIL"} else {"WARN"});
            false
        }
    }
}

pub fn doctor() -> Result<()> {
    println!("Wyldtone SDK toolchain check");
    let mut okay = true;
    for (name,flag) in [("em++","--version"),("rustc","--version"),("cargo","--version"),("cmake","--version"),("ninja","--version")] {
        okay &= tool(name,flag,true);
    }
    if in_path("emcmake") { println!("[OK] emcmake: executable found"); }
    else {println!("[FAIL] emcmake: not in PATH (required for JUCE builds)"); okay=false;}
    tool("faust","-v",false);
    let juce = env::var_os("JUCE_DIR").map(PathBuf::from);
    match juce {
        Some(path) if path.join("modules/juce_audio_processors_headless").is_dir() => {
            println!("[OK] JUCE checkout: {} (externally supplied)",path.display());
        }
        Some(path) => println!("[WARN] JUCE checkout not found/unsupported at {} (optional for C++ and Faust)",path.display()),
        None => println!("[WARN] JUCE_DIR not set (required only for JUCE builds)"),
    }
    if !okay {return Err("One or more required SDK build tools are unavailable".into());}
    Ok(())
}

fn check_manifest(manifest: &PluginManifest) -> Result<()> {
    if manifest.format_version != MANIFEST_FORMAT_VERSION {
        return Err(format!("Unsupported WAP manifest version {} (expected {})",manifest.format_version, MANIFEST_FORMAT_VERSION).into());
    }
    for (field,value) in [("id",&manifest.plugin.id),("name",&manifest.plugin.name),("manufacturer",&manifest.plugin.manufacturer),("version",&manifest.plugin.version)] {
        if value.trim().is_empty() {return Err(format!("Manifest plugin.{field} must not be empty").into());}
    }
    let mut ids = HashSet::new();
    if let Some(category) = manifest.plugin.category.as_deref() {
        if normalize_category(category).is_none() {
            return Err(category_error(category).into());
        }
    }
    for param in &manifest.parameters {
        if param.id.trim().is_empty() || !ids.insert(param.id.as_str()) {
            return Err(format!("Empty or duplicate manifest parameter ID: {}",param.id).into());
        }
    }
    Ok(())
}

struct ValidatedWap {
    manifest: PluginManifest,
    parameters: Vec<Parameter>,
    buses: Option<(Vec<usize>,Vec<usize>)>,
}

fn validate_package(path: &Path) -> Result<ValidatedWap> {
    if fs::metadata(path)?.len() > 64 * 1024 * 1024 {return Err("WAP exceeds the 64 MiB CLI limit".into());}
    let wap = load_wap(path)?;
    check_manifest(&wap.manifest)?;
    // Validate artwork with the same strict helper used by the SDK package reader.
    let _ = load_wap_icon(path)?;
    let engine = WasmPlugin::create_engine()?;
    let mut plugin = WasmPlugin::from_bytes(&engine,&wap.wasm)?;
    let params = plugin.load_parameters()?;
    wap.manifest.validate_against(&params)?;
    let buses = plugin.bus_layout_channels()?;
    Ok(ValidatedWap {manifest:wap.manifest, parameters:params, buses})
}

fn describe_buses(buses: &Option<(Vec<usize>,Vec<usize>)>) -> String {
    match buses {
        Some((input,output)) => format!("{:?} -> {:?}",input,output),
        None => "legacy ABI (no explicit bus declarations)".to_string(),
    }
}

fn print_parameters(params: &[Parameter]) {
    println!("Parameters ({}):",params.len());
    for p in params {println!("  {:<24} {} {:?}",p.id,p.name,p.kind);}
}

pub fn inspect(path: &Path) -> Result<()> {
    let ext = path.extension().and_then(|s|s.to_str()).unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "wap" => {
            let checked=validate_package(path)?;
            let plugin=&checked.manifest.plugin;
            println!("{} {} by {}",plugin.name,plugin.version,plugin.manufacturer);
            println!("ID: {} | Category: {} | WAP manifest: {}",plugin.id,plugin.category.as_deref().unwrap_or("n/a"),checked.manifest.format_version);
            println!("Audio buses: {}", describe_buses(&checked.buses));
            print_parameters(&checked.parameters);
        }
        "wasm" => {
            let engine=WasmPlugin::create_engine()?;
            let mut plugin=WasmPlugin::from_file(&engine,path)?;
            let params=plugin.load_parameters()?;
            println!("WASM: {}",path.display());
            println!("Audio buses: {}",describe_buses(&plugin.bus_layout_channels()?));
            print_parameters(&params);
        }
        _ => return Err("inspect accepts a .wap package or .wasm module".into()),
    }
    Ok(())
}

pub fn validate(path: &Path) -> Result<()> {
    if !path.extension().and_then(|s|s.to_str()).is_some_and(|s|s.eq_ignore_ascii_case("wap")) {
        return Err("validate expects a .wap package".into());
    }
    let checked=validate_package(path)?;
    println!("PASS: {} {} ({})",checked.manifest.plugin.name,checked.manifest.plugin.version,checked.manifest.plugin.id);
    println!("  Manifest version: {}",checked.manifest.format_version);
    println!("  Parameters: {}",checked.parameters.len());
    println!("  Audio buses: {}",describe_buses(&checked.buses));
    println!("Note: structural/runtime validation only; not a malware or real-time safety audit.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize,Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn test_dir(name:&str)->PathBuf {
        env::temp_dir().join(format!("wyldtone-cli-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)))
    }
    #[test]
    fn rejects_invalid_new_arguments() {
        assert!(parse_new(&["9bad".into(),"--template".into(),"cpp".into()]).is_err());
        assert!(parse_new(&["Good".into(),"--template".into(),"missing".into()]).is_err());
        assert!(parse_new(&["Good".into()]).is_err());
        assert!(parse_new(&["Good".into(),"--template".into(),"faust".into(),"--template".into(),"cpp".into()]).is_err());
    }
    #[test]
    fn creates_each_template_without_overwriting() {
        for template in ["cpp","juce","faust"] {
            let base=test_dir(template);
            fs::create_dir(&base).unwrap();
            let path=base.join("NewEffect");
            let args=vec![path.to_str().unwrap().to_string(),"--template".into(),template.into()];
            new(&args).unwrap();
            let config:serde_json::Value=serde_json::from_slice(&fs::read(path.join("plugin.config.json")).unwrap()).unwrap();
            assert_eq!(config["name"],"NewEffect");
            assert_eq!(config["audio"]["inputs"][0],2);
            assert!(config.get(template).is_some());
            assert!(new(&args).is_err());
            fs::remove_dir_all(base).unwrap();
        }
    }
    #[test]
    fn rejects_duplicate_manifest_parameters() {
        let data = r#"{"format_version":1,"plugin":{"id":"example.x","name":"Example","manufacturer":"Dev","version":"0.1"},"parameters":[{"id":"x","name":"X","label":"","type":"float","min":0,"max":1,"default":{"type":"float","value":0.5},"step":null,"automatable":true,"choices":null},{"id":"x","name":"X","label":"","type":"float","min":0,"max":1,"default":{"type":"float","value":0.5},"step":null,"automatable":true,"choices":null}]}"#;
        let manifest:PluginManifest=serde_json::from_str(data).unwrap();
        assert!(check_manifest(&manifest).is_err());
    }
}
