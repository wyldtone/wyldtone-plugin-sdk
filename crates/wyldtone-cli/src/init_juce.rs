//! Conservative, source-only JUCE plugin.config.json generator.
//! It never edits upstream project files other than creating a new config.
use std::{error::Error, fs, path::{Path, PathBuf}};
use serde_json::{json, Value};
use wyldtone_plugin_api::categories::{normalize_category,category_error,CATEGORIES};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Options {
    project: PathBuf,
    id: String,
    name: String,
    description: String,
    processor: Option<String>,
    category: String,
}

fn parse(args: &[String]) -> Result<Options> {
    let (project, flags) = args.split_first().ok_or(
        "Usage: wyldtone init-juce <project-dir> --id ID --name NAME --description TEXT --category CATEGORY [--processor CLASS]"
    )?;
    if project.starts_with('-') { return Err("Project directory must be the first argument".into()); }
    let mut id = None;
    let mut name = None;
    let mut description = None;
    let mut processor = None;
    let mut category = None;
    let mut i = 0;
    while i < flags.len() {
        let value = flags.get(i + 1).ok_or_else(|| format!("Missing value for {}", flags[i]))?;
        if value.trim().is_empty() { return Err(format!("{} cannot be empty", flags[i]).into()); }
        match flags[i].as_str() {
            "--id" if id.is_none() => id = Some(value.clone()),
            "--name" if name.is_none() => name = Some(value.clone()),
            "--description" if description.is_none() => description = Some(value.clone()),
            "--processor" if processor.is_none() => processor = Some(value.clone()),
            "--category" if category.is_none() => category = Some(normalize_category(value).ok_or_else(|| category_error(value))?.to_owned()),
            _ => return Err(format!("Unknown or duplicate option: {}", flags[i]).into()),
        }
        i += 2;
    }
    let id = id.ok_or("--id required")?;
    if id.len() > 200 || !id.contains('.') ||
       !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
    { return Err("ID must be a dotted ASCII identifier (max 200 chars)".into()); }
    Ok(Options {
        project: PathBuf::from(project), id,
        name: name.ok_or("--name required")?,
        description: description.ok_or("--description required")?, processor,
        category: category.ok_or_else(|| format!("--category required. Available categories: {}", CATEGORIES.join(", ")))?,
    })
}

fn sources(project: &Path) -> Result<Vec<PathBuf>> {
    // Only the usual processor locations; intentionally avoid .git, build, tests.
    let mut dirs = vec![project.to_path_buf()];
    if project.join("src").is_dir() { dirs.push(project.join("src")); }
    let mut files = Vec::new();
    for dir in dirs {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()).is_some_and(|e|
                    matches!(e, "h" | "hpp" | "cpp")) {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

fn detect_processor(project: &Path, candidates: &[PathBuf], selected: Option<&str>) -> Result<(String, String)> {
    let mut found = Vec::new();
    for path in candidates {
        if !matches!(path.extension().and_then(|e| e.to_str()), Some("h" | "hpp")) { continue; }
        let content = fs::read_to_string(path)?;
        for segment in content.split("class ").skip(1) {
            let Some(open) = segment.find('{') else { continue };
            let definition = &segment[..open];
            if !definition.contains("public") || !definition.contains("AudioProcessor") { continue; }
            let Some(colon) = definition.find(':') else { continue };
            let class = definition[..colon].trim().split_whitespace().next().unwrap_or("");
            let inheritance = &definition[colon + 1..];
            // Match complete base-class identifiers, never substrings:
            // AudioProcessorEditor is an editor, not an AudioProcessor.
            let is_processor = inheritance.split(',').any(|base| {
                let parts: Vec<_> = base.split_whitespace().collect();
                parts.iter().any(|part| *part == "public") &&
                    parts.iter().any(|part| *part == "juce::AudioProcessor" || *part == "AudioProcessor")
            });
            if !is_processor { continue; }
            if !class.is_empty() && class.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                let relative = path.strip_prefix(project)?.to_string_lossy().replace('\\', "/");
                found.push((class.to_owned(), relative));
            }
        }
    }
    found.sort();
    found.dedup();
    if let Some(selected) = selected {
        let matches: Vec<_> = found.into_iter().filter(|(class, _)| class == selected).collect();
        return match matches.as_slice() {
            [(class, header)] => Ok((class.clone(), header.clone())),
            [] => Err(format!("Requested AudioProcessor class '{selected}' not found (check spelling and inheritance).").into()),
            _ => Err(format!("AudioProcessor class '{selected}' appears in multiple headers: {:?}; resolve ambiguity manually.", matches).into()),
        };
    }
    match found.as_slice() {
        [(class, header)] => Ok((class.clone(), header.clone())),
        [] => Err("Could not uniquely detect a class derived from juce::AudioProcessor in a root/src header. Specify processor/header manually.".into()),
        _ => Err(format!("Multiple AudioProcessor classes found ({:?}); choose one manually.", found).into()),
    }
}

fn cmake_value(contents: &str, keyword: &str) -> Option<String> {
    // Read simple quoted or unquoted CMake arguments, not a general CMake evaluator.
    let at = contents.find(keyword)?;
    let tail = contents[at + keyword.len()..].trim_start();
    if let Some(rest) = tail.strip_prefix('"') {
        Some(rest.split('"').next()?.to_owned())
    } else {
        let value = tail.split(|c:char| c.is_whitespace() || c == ')').next()?;
        if value.contains('$') { None } else { Some(value.to_owned()) }
    }
}

fn detect_version(project: &Path) -> Option<String> {
    let cmake = fs::read_to_string(project.join("CMakeLists.txt")).ok()?;
    // Read VERSION from project(...) specifically, not from an unrelated dependency.
    let start = cmake.find("project(")?;
    let block = cmake[start..].split(')').next()?;
    let tokens: Vec<_> = block.split_whitespace().collect();
    let i = tokens.iter().position(|x| *x == "VERSION")?;
    let version = tokens.get(i + 1)?.trim_matches('"');
    if version.chars().all(|c| c.is_ascii_digit() || c == '.') { Some(version.to_string()) }
    else { None }
}

fn detect_manufacturer(project: &Path) -> Option<String> {
    let cmake = fs::read_to_string(project.join("CMakeLists.txt")).ok()?;
    cmake_value(&cmake, "COMPANY_NAME")
}

fn detect_audio(project: &Path) -> Option<Value> {
    let path = [project.join("src/PluginProcessor.cpp"),project.join("PluginProcessor.cpp")]
        .into_iter().find(|p| p.is_file())?;
    let s = fs::read_to_string(path).ok()?;
    fn side(s: &str, method: &str) -> Option<u32> {
        let at = s.find(method)?;
        let tail = &s[at + method.len()..];
        let next_side = tail.find(".with").unwrap_or(tail.len());
        let call = &tail[..next_side];
        let set = call.find("AudioChannelSet::")?;
        let channels = &call[set + "AudioChannelSet::".len()..];
        if channels.starts_with("stereo()") { Some(2) }
        else if channels.starts_with("mono()") { Some(1) }
        else { None }
    }
    Some(json!({"inputs":[side(&s, ".withInput(")?],"outputs":[side(&s, ".withOutput(")?]}))
}

fn detect_modules_and_includes(project: &Path) -> (Vec<String>, Vec<String>) {
    let mut modules = Vec::new();
    let mut includes = Vec::new();
    // Conservative header-driven discovery for modules known to work with headless JUCE.
    let mut stack = vec![project.join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !path.is_symlink() { stack.push(path); }
            else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e,"h"|"hpp"|"cpp")) {
                if let Ok(s) = fs::read_to_string(path) {
                    if s.contains("#include <juce_dsp/") && !modules.contains(&"juce_dsp".to_owned()) {
                        modules.push("juce_dsp".to_owned());
                    }
                    for (needle, dir) in [
                        ("#include <chowdsp_wdf/", "libs/chowdsp_wdf/include"),
                        ("#include <xsimd/", "libs/xsimd/include"),
                    ] {
                        if s.contains(needle) && project.join(dir).is_dir() && !includes.contains(&dir.to_owned()) {
                            includes.push(dir.to_owned());
                        }
                    }
                }
            }
        }
    }
    modules.sort();
    includes.sort();
    (modules, includes)
}

fn generate(opts: &Options) -> Result<Value> {
    if !opts.project.is_dir() { return Err(format!("Project directory not found: {}", opts.project.display()).into()); }
    let (processor, header) = detect_processor(&opts.project, &sources(&opts.project)?, opts.processor.as_deref())?;
    let (modules, include_dirs) = detect_modules_and_includes(&opts.project);
    let version = detect_version(&opts.project).unwrap_or_else(|| {
        eprintln!("Warning: version not detected; using 0.1.0 (review before publishing)");
        "0.1.0".into()
    });
    let manufacturer = detect_manufacturer(&opts.project).unwrap_or_else(|| {
        eprintln!("Warning: manufacturer not detected; using Unknown (review before publishing)");
        "Unknown".into()
    });
    let audio = detect_audio(&opts.project);
    if audio.is_none() { eprintln!("Warning: audio buses not statically recognized; omitted (review before building)"); }
    let mut json = json!({
        "id": opts.id, "name": opts.name, "description": opts.description,
        "category": opts.category,
        "manufacturer": manufacturer, "version": version,
        "juce": {
            "processor": processor, "header": header,
            "modules": modules, "include_dirs": include_dirs,
            "simd128": false, "mode": "headless"
        }
    });
    if let Some(audio) = audio { json["audio"] = audio; }
    Ok(json)
}

pub fn run(args: &[String]) -> Result<()> {
    let opts = parse(args)?;
    let output = opts.project.join("plugin.config.json");
    if output.exists() {
        return Err(format!("Refusing to overwrite existing config: {}", output.display()).into());
    }
    let config = generate(&opts)?;
    // Exclusive creation protects manually maintained configs even across races.
    use std::io::Write;
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&output)?;
    file.write_all(&serde_json::to_vec_pretty(&config)?)?;
    file.write_all(b"\n")?;
    println!("Generated: {}", output.display());
    println!("Review manufacturer, version, buses, modules and third-party includes before building.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize,Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn fixture(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("wyldtone-init-juce-{}-{}-{}",name,std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir_all(p.join("src/dsp")).unwrap();
        p
    }
    #[test]
    fn infers_existing_juce_project_and_does_not_overwrite() {
        let p = fixture("basic");
        fs::write(p.join("CMakeLists.txt"), "project(Test VERSION 1.2.3)\njuce_add_plugin(Test COMPANY_NAME \"A Company\" FORMATS VST3)").unwrap();
        fs::write(p.join("src/PluginProcessor.h"), "class ExistingAudioProcessor : public juce::AudioProcessor { };").unwrap();
        fs::write(p.join("src/PluginProcessor.cpp"), "AudioProcessor(BusesProperties().withInput(\"Input\", juce::AudioChannelSet::stereo(), true).withOutput(\"Output\", juce::AudioChannelSet::stereo(), true))").unwrap();
        fs::write(p.join("src/dsp/Chain.h"), "#include <juce_dsp/juce_dsp.h>").unwrap();
        let args = vec![p.display().to_string(),"--id".into(),"org.example.test".into(),"--name".into(),"Example".into(),"--description".into(),"My plugin".into(),"--category".into(),"EQ".into()];
        run(&args).unwrap();
        let json:Value = serde_json::from_slice(&fs::read(p.join("plugin.config.json")).unwrap()).unwrap();
        assert_eq!(json["juce"]["processor"],"ExistingAudioProcessor");
        assert_eq!(json["juce"]["header"],"src/PluginProcessor.h");
        assert_eq!(json["manufacturer"],"A Company");
        assert_eq!(json["version"],"1.2.3");
        assert_eq!(json["audio"]["inputs"][0],2);
        assert_eq!(json["juce"]["modules"][0],"juce_dsp");
        assert!(run(&args).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn rejects_missing_or_ambiguous_processor() {
        let p = fixture("ambiguous");
        fs::write(p.join("src/PluginProcessor.h"), "class A : public juce::AudioProcessor {}; class B : public juce::AudioProcessor {};").unwrap();
        assert!(detect_processor(&p, &sources(&p).unwrap(), None).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn editor_class_does_not_count_as_audio_processor() {
        let p = fixture("editor");
        fs::write(p.join("src/PluginProcessor.h"),
            "class MonarchAudioProcessor : public juce::AudioProcessor { };").unwrap();
        fs::write(p.join("src/PluginEditor.h"),
            "class MonarchAudioProcessorEditor : public juce::AudioProcessorEditor { };").unwrap();
        let (class, header) = detect_processor(&p, &sources(&p).unwrap(), None).unwrap();
        assert_eq!(class, "MonarchAudioProcessor");
        assert_eq!(header, "src/PluginProcessor.h");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn explicit_processor_selects_from_multiple_real_processors() {
        let p = fixture("selection");
        fs::write(p.join("src/PluginProcessor.h"),
            "class A : public juce::AudioProcessor {}; class B : public juce::AudioProcessor {};").unwrap();
        let files = sources(&p).unwrap();
        assert_eq!(detect_processor(&p, &files, Some("B")).unwrap().0, "B");
        assert!(detect_processor(&p, &files, Some("Missing")).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn accepts_processor_flag_and_rejects_duplicate() {
        let args = ["projects/test", "--id", "org.example.test", "--name", "Example",
            "--description", "A test", "--category", "Drive", "--processor", "MonarchAudioProcessor"]
            .iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(parse(&args).unwrap().processor.as_deref(), Some("MonarchAudioProcessor"));
        let mut duplicates = args.clone();
        duplicates.extend(["--processor".to_string(), "Other".to_string()]);
        assert!(parse(&duplicates).is_err());
    }
    #[test]
    fn category_validation_and_other_fallback() {
        let mut args = vec!["projects/test".into(), "--id".into(), "org.example.test".into(),
            "--name".into(), "Test".into(), "--description".into(), "Plugin".into(),
            "--category".into(), "Other".into()];
        assert_eq!(parse(&args).unwrap().category, "Other");
        *args.last_mut().unwrap() = "Super Heavy Drive".into();
        assert!(parse(&args).unwrap_err().to_string().contains("Available categories"));
    }
    #[test]
    fn rejects_bad_args() {
        assert!(parse(&[".".into(),"--id".into(),"BAD".into(),"--name".into(),"x".into(),"--description".into(),"x".into()]).is_err());
        assert!(parse(&[".".into(),"--id".into(),"org.test".into(),"--id".into(),"org.test".into(),"--name".into(),"x".into(),"--description".into(),"x".into()]).is_err());
    }
}
