use std::{env, error::Error, fs, path::{Path, PathBuf}, process::Command};
use wyldtone_plugin_api::config::JuceConfig;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn run(command: &mut Command, label: &str) -> Result<()> {
    println!("{label}: {command:?}");
    let status = command.status()?;
    if !status.success() { return Err(format!("{label} failed: {status}").into()); }
    Ok(())
}

// CMake/Ninja cannot use the Windows extended-length \\?\ prefix in generated
// object paths. Convert canonical paths to regular, slash-separated paths
// before handing them to CMake. This does not modify files on disk.
fn cmake_path(path: &Path) -> Result<String> {
    let absolute = fs::canonicalize(path)?;
    let value = absolute.to_string_lossy();

    #[cfg(windows)]
    {
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            return Ok(format!("//{}", unc.replace('\\', "/")));
        }
        if let Some(local) = value.strip_prefix(r"\\?\") {
            return Ok(local.replace('\\', "/"));
        }
    }

    Ok(value.replace('\\', "/"))
}

// Locate a method using its C++ qualified name and replace only its body.
// For v0.3 reject ambiguous signatures. Never edit developer source files.
fn replace_method_body(src: &str, class: &str, method: &str, replacement: &str) -> Result<String> {
    let signature = format!("{class}::{method}");
    let occurrences: Vec<usize> = src.match_indices(&signature).map(|(i,_)| i).collect();
    if occurrences.len() != 1 { return Err(format!("Expected one definition of {signature}, found {}. Automatic headless conversion refused.", occurrences.len()).into()); }
    let after = occurrences[0] + signature.len();
    let open = src[after..].find('{').map(|x| x+after).ok_or("No method body")?;
    // Reject declarations, lambdas or preprocessor branches between signature and body.
    let header = &src[after..open];
    if header.contains(';') || header.contains('#') { return Err(format!("Unsupported signature for {signature}").into()); }
    let bytes=src.as_bytes();
    let (mut depth,mut end)=(0u32,None);
    // C++ bodies can contain strings/comments; a lexical brace scanner handles them.
    let mut i=open;
    let mut state=0u8; // 0 normal, 1 quoted string, 2 char, 3 line comment, 4 block comment
    while i < bytes.len() {
        let a=bytes[i];let b=bytes.get(i+1).copied().unwrap_or(0);
        match state {
            0 if a==b'"' =>state=1,
            0 if a==b'\'' =>state=2,
            0 if a==b'/' && b==b'/' =>{state=3;i+=1;},
            0 if a==b'/' && b==b'*' =>{state=4;i+=1;},
            0 if a==b'{' =>depth+=1,
            0 if a==b'}' =>{depth-=1;if depth==0{end=Some(i);break;}},
            1|2 if a==b'\\' =>i+=1,
            1 if a==b'"' =>state=0,
            2 if a==b'\'' =>state=0,
            3 if a==b'\n' =>state=0,
            4 if a==b'*' && b==b'/' =>{state=0;i+=1;},
            _=>{}
        }
        i+=1;
    }
    let end=end.ok_or("Unterminated C++ method")?;
    Ok(format!("{}{{\n    {}\n}}{}",&src[..open],replacement,&src[end+1..]))
}

// Header-only JUCE projects put the AudioProcessor implementation directly
// inside the class definition, rather than in Class::method() definitions.
// Only the staged header is rewritten; never modify a developer's sources.
fn method_parentheses(src: &str, method: &str) -> Vec<(usize, usize)> {
    src.match_indices(method).filter_map(|(start, _)| {
        if start > 0 && (src.as_bytes()[start - 1].is_ascii_alphanumeric()
            || src.as_bytes()[start - 1] == b'_') {
            return None;
        }
        let mut open = start + method.len();
        while src.as_bytes().get(open).is_some_and(u8::is_ascii_whitespace) {
            open += 1;
        }
        (src.as_bytes().get(open) == Some(&b'(')).then_some((start, open))
    }).collect()
}

fn find_inline_method_openings(src: &str, method: &str) -> Vec<usize> {
    let mut openings = Vec::new();
    for (start, open_paren) in method_parentheses(src, method) {
        let args_start = open_paren + 1;
        // createEditor() and hasEditor() have no arguments; other overloads
        // are intentionally rejected instead of guessing how to transform them.
        let Some(close_rel) = src[args_start..].find(')') else { continue };
        let close = args_start + close_rel;
        if !src[args_start..close].trim().is_empty() { continue; }
        let tail = &src[close + 1..];
        let Some(open_rel) = tail.find('{') else { continue };
        let header = &tail[..open_rel];
        if header.contains(';') || header.contains('#') || header.contains('}')
            || header.len() > 160 {
            continue;
        }
        // Avoid matching calls like processor.createEditor(). An in-class
        // definition has a return type on the same line before the method.
        let line = &src[src[..start].rfind('\n').map_or(0, |i| i + 1)..start];
        if line.contains("->") || line.contains('.') || line.trim().is_empty() {
            continue;
        }
        openings.push(close + 1 + open_rel);
    }
    openings
}

#[cfg(test)]
fn replace_inline_method_body(src: &str, method: &str, replacement: &str) -> Result<String> {
    let openings = find_inline_method_openings(src, method);
    if openings.len() != 1 {
        return Err(format!(
            "Expected one inline definition of {method}(), found {}. Headless conversion refused.",
            openings.len()
        ).into());
    }
    replace_braced_body(src, openings[0], replacement)
}

// Shared brace scanner for inline methods. Ignores braces within C++ string
// literals, character literals and comments.
fn replace_braced_body(src: &str, open: usize, replacement: &str) -> Result<String> {
    let bytes = src.as_bytes();
    let (mut depth, mut end) = (0u32, None);
    let mut i = open;
    let mut state = 0u8;
    while i < bytes.len() {
        let a = bytes[i];
        let b = bytes.get(i + 1).copied().unwrap_or(0);
        match state {
            0 if a == b'"' => state = 1,
            0 if a == b'\'' => state = 2,
            0 if a == b'/' && b == b'/' => { state = 3; i += 1; },
            0 if a == b'/' && b == b'*' => { state = 4; i += 1; },
            0 if a == b'{' => depth += 1,
            0 if a == b'}' => {
                depth -= 1;
                if depth == 0 { end = Some(i); break; }
            },
            1 | 2 if a == b'\\' => i += 1,
            1 if a == b'"' => state = 0,
            2 if a == b'\'' => state = 0,
            3 if a == b'\n' => state = 0,
            4 if a == b'*' && b == b'/' => { state = 0; i += 1; },
            _ => {}
        }
        i += 1;
    }
    let end = end.ok_or("Unterminated C++ method")?;
    Ok(format!("{}{{\n    {}\n}}{}", &src[..open], replacement, &src[end + 1..]))
}

// Headless conversion is per METHOD, not per project. A legitimate JUCE
// processor can define hasEditor() inline in its header and createEditor()
// in a .cpp (or vice versa). Some projects put one method into the GUI .cpp,
// which this SDK intentionally does not compile.
//
// Each method is handled in this order:
// 1. Replace exactly one out-of-class implementation in a staged .cpp.
// 2. Otherwise replace exactly one inline implementation in the processor header.
// 3. Otherwise synthesize a definition only if the header declares the method.
// Ambiguous/missing declarations fail closed rather than modifying source code
// we do not understand. The developer's original files are never modified.
fn declared_method_in_header(src: &str, method: &str) -> bool {
    method_parentheses(src, method).iter().any(|&(_, open)| {
        let rest = &src[open + 1..];
        if !rest.starts_with(')') { return false; }
        let rest = &rest[1..];
        // A declaration has a ';' before any opening brace. This excludes
        // inline bodies and most call sites; avoid unsupported macros.
        let semicolon = rest.find(';');
        let brace = rest.find('{');
        matches!(semicolon, Some(s) if brace.map_or(true, |b| s < b) && s < 160
            && !rest[..s].contains('#'))
    })
}

fn rewrite_headless_method(
    header_path: &Path, cpp_sources: &[PathBuf],
    class: &str, method: &str, replacement: &str,
) -> Result<Option<String>> {
    let signature = format!("{class}::{method}");
    let mut matches = Vec::new();
    for path in cpp_sources {
        let text = fs::read_to_string(path)?;
        if text.contains(&signature) {
            matches.push(path.clone());
        }
    }
    if matches.len() > 1 {
        return Err(format!(
            "Multiple compiled C++ sources define {signature}: {:?}. Headless conversion refused.",
            matches
        ).into());
    }
    if let Some(path) = matches.pop() {
        let source = fs::read_to_string(&path)?;
        let converted = replace_method_body(&source, class, method, replacement)?;
        fs::write(path, converted)?;
        return Ok(None);
    }

    let header = fs::read_to_string(header_path)?;
    let inline_openings = find_inline_method_openings(&header, method);
    match inline_openings.as_slice() {
        [open] => {
            let converted = replace_braced_body(&header, *open, replacement)?;
            fs::write(header_path, converted)?;
            return Ok(None);
        }
        [] if declared_method_in_header(&header, method) => {}
        [] => {
            return Err(format!(
                "Cannot locate a {method}() implementation or declaration in {}. \
                 Neither the JUCE checkout nor the plugin checkout were changed.",
                header_path.display()
            ).into());
        }
        _ => return Err(format!(
            "Ambiguous inline {method}() definitions in {}. Headless conversion refused.",
            header_path.display()
        ).into()),
    }
    // The declaration exists, but its definition may be in a GUI source file
    // excluded from WASM. Supply one standalone implementation in staging.
    let method_body = match method {
        "hasEditor" => format!("bool {class}::hasEditor() const {{ {replacement} }}"),
        "createEditor" => format!(
            "juce::AudioProcessorEditor* {class}::createEditor() {{ {replacement} }}"
        ),
        _ => return Err(format!("Unknown headless method: {method}").into()),
    };
    // Leave the generated file beside the staged processor header so local
    // includes resolve; the CMake source glob collects it automatically.
    Ok(Some(method_body))
}

fn stage_source(plugin_dir: &Path, staging: &Path, processor: &str, header: &str) -> Result<bool> {
    let has_processor_cpp = plugin_dir.join("PluginProcessor.cpp").is_file()
        || plugin_dir.join("src/PluginProcessor.cpp").is_file();
    let header_only = !has_processor_cpp;
    let header_path = Path::new(header);
    if header_path.is_absolute()
        || header_path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("JUCE header path must be relative to the plugin directory".into());
    }
    let header_relative = if plugin_dir.join(header_path).is_file() {
        header_path.to_path_buf()
    } else if plugin_dir.join("src").join(header_path).is_file() {
        Path::new("src").join(header_path)
    } else {
        return Err(format!("JUCE processor header not found: {header}").into());
    };
    if staging.exists() { fs::remove_dir_all(staging)?; }
    fs::create_dir_all(staging)?;
    let mut count = 0;
    let mut cpp_sources = Vec::new();
    let mut directories = vec![PathBuf::new()];
    while let Some(relative_dir) = directories.pop() {
        let original_dir = plugin_dir.join(&relative_dir);
        for entry in fs::read_dir(&original_dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let relative = relative_dir.join(name.as_ref());
            if path.is_dir() {
                if entry.file_type()?.is_symlink() { continue; }
                if (relative_dir.as_os_str().is_empty() && name == "src")
                    || (relative_dir.starts_with("src")
                        && !matches!(name.as_ref(), "build" | ".git" | "tests")) {
                    directories.push(relative);
                }
                continue;
            }
            if !path.is_file() || entry.file_type()?.is_symlink() { continue; }
            if !matches!(path.extension().and_then(|s| s.to_str()),
                Some("cpp" | "cc" | "cxx" | "c" | "h" | "hpp" | "inl" | "ipp" | "tpp")) {
                continue;
            }
            if name == "PluginEditor.cpp" { continue; }
            let dst = staging.join(&relative);
            if let Some(parent) = dst.parent() { fs::create_dir_all(parent)?; }
            if name == "PluginProcessor.cpp" {
                // Drop the GUI-only editor include, but leave all DSP source
                // unchanged until each method is handled below.
                let source = fs::read_to_string(&path)?;
                let source = source.lines()
                    .filter(|line| !matches!(line.trim(),
                        "#include \"PluginEditor.h\"" | "#include <PluginEditor.h>"))
                    .collect::<Vec<_>>().join("\n");
                fs::write(&dst, source)?;
            } else {
                fs::copy(path, &dst)?;
            }
            if matches!(dst.extension().and_then(|x| x.to_str()),
                Some("cpp" | "cc" | "cxx")) {
                cpp_sources.push(dst);
            }
            count += 1;
        }
    }
    if count == 0 { return Err("No C++/header source files found for JUCE project".into()); }
    let staged_header = staging.join(&header_relative);
    if !staged_header.is_file() {
        return Err(format!("Staged JUCE processor header missing: {}", header_relative.display()).into());
    }
    let mut synthesized = Vec::new();
    for (method, replacement) in [
        ("hasEditor", "return false;"),
        ("createEditor", "return nullptr;"),
    ] {
        if let Some(body) = rewrite_headless_method(
            &staged_header, &cpp_sources, processor, method, replacement
        )? {
            synthesized.push(body);
        }
    }
    if !synthesized.is_empty() {
        let parent = staged_header.parent().ok_or("Invalid staged JUCE header")?;
        // This file is linked like the other plugin sources, and it is NOT
        // placed in the developer checkout.
        let generated = format!(
            "#include \"JuceHeader.h\"\n#include \"{}\"\n{}\n",
            staged_header.file_name().ok_or("Invalid processor header name")?.to_string_lossy(),
            synthesized.join("\n")
        );
        fs::write(parent.join("WyldtoneHeadlessEditor.cpp"), generated)?;
    }
    Ok(header_only)
}

pub fn build(config: &JuceConfig, name: &str, plugin_dir: &Path, wasm: &Path) -> Result<()> {
    let juce_dir = env::var_os("JUCE_DIR").map(PathBuf::from)
        .ok_or("Set JUCE_DIR to your local JUCE checkout (official upstream or legacy patched)")?;
    if !juce_dir.join("modules/juce_core").is_dir() {return Err("JUCE_DIR does not contain modules/juce_core".into());}
    let mode=env::var("WYLDTONE_JUCE_MODE").unwrap_or_else(|_|"upstream".into());
    if mode != "upstream" { return Err("Only an unmodified external JUCE checkout (upstream) is supported".into()); }
    let out=wasm.parent().ok_or("Invalid WASM output path")?;
    let staging=out.join("juce-staged-source");
    let header_only = stage_source(plugin_dir, &staging, &config.processor, &config.header)?;
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../adapters/juce");
    let generated = out.join("PluginEntry.cpp");
    // Headless example headers may use unqualified JUCE types (PIP examples
    // normally rely on a JUCE-generated umbrella header/using directive).
    // This using directive is isolated to the generated entry translation unit.
    let juce_header = "juce_audio_processors_headless";
    let entry = if header_only {
        format!(
            "#include \"JuceHeader.h\"\n#include <memory>\nusing namespace juce;\n#include \"{}\"\nstd::unique_ptr<juce::AudioProcessor> createPlugin() {{ return std::make_unique<{}>(); }}\n",
            config.header, config.processor
        )
    } else {
        format!(
            "#include <{juce_header}/{juce_header}.h>\n#include <memory>\n#include \"{}\"\nstd::unique_ptr<juce::AudioProcessor> createPlugin() {{ return std::make_unique<{}>(); }}\n",
            config.header, config.processor
        )
    };
    fs::write(generated, entry)?;
    let generated_dir = out.join("generated");
    fs::create_dir_all(&generated_dir)?;
    // APVTS refers to ValueTree, Value and UndoManager in its class declaration.
    // Include juce_data_structures BEFORE the stand-alone APVTS header; the
    // headless processor module does not bring these declarations into scope.
    let mut contents=format!(
        "#pragma once\n#include <{juce_header}/{juce_header}.h>\n#include <juce_data_structures/juce_data_structures.h>\n"
    );
    // APVTS is part of the GUI-bearing juce_audio_processors module but KFuzz
    // only uses its parameter/state subset. The SDK provides its headless
    // implementation and VFS compatibility layer at link time.
    contents.push_str("#include <juce_audio_processors/utilities/juce_AudioProcessorValueTreeState.h>\n");
    for module in &config.modules {
        if !module.starts_with("juce_") || !module.chars().all(|c| c.is_ascii_alphanumeric() || c=='_') || !juce_dir.join("modules").join(module).is_dir() {
            return Err(format!("Unknown JUCE module: {module}").into());
        }
        contents.push_str(&format!("#include <{module}/{module}.h>\n"));
    }
    fs::write(generated_dir.join("JuceHeader.h"), contents)?;
    // Resolve include roots against the original checkout, not the staged
    // source tree. Third-party libraries stay under developer control.
    let mut include_dirs = Vec::new();
    for include in &config.include_dirs {
        let relative = Path::new(include);
        if relative.is_absolute() || relative.components().any(|component|
            matches!(component, std::path::Component::ParentDir | std::path::Component::Prefix(_))) {
            return Err(format!("JUCE include path must be relative to the plugin root: {include}").into());
        }
        let directory = plugin_dir.join(relative);
        if !directory.is_dir() {
            return Err(format!("JUCE include directory missing: {} (check git submodules)",
                directory.display()).into());
        }
        include_dirs.push(cmake_path(&directory)?);
    }
    let cmake_dir = format!("{}/juce-cmake-build", cmake_path(out)?);
    let mut cfg=Command::new("emcmake");
    cfg.arg("cmake").arg("-S").arg(cmake_path(&sdk.join("build"))?)
       .arg("-B").arg(&cmake_dir).arg("-G").arg("Ninja")
       .arg(format!("-DJUCE_DIR={}",cmake_path(&juce_dir)?))
       .arg(format!("-DPLUGIN_DIR={}",cmake_path(&staging)?))
       .arg(format!("-DWYLDTONE_OUTPUT_DIR={}",cmake_path(out)?))
       .arg(format!("-DWYLDTONE_PLUGIN_NAME={name}"))
       .arg(format!("-DWYLDTONE_JUCE_MODE={mode}"))
       .arg(format!("-DWYLDTONE_JUCE_MODULES={}",config.modules.join(";")))
       .arg(format!("-DWYLDTONE_PLUGIN_EXTRA_INCLUDES={}", include_dirs.join(";")))
       .arg(format!("-DWYLDTONE_ENABLE_SIMD128={}", if config.simd128 { "ON" } else { "OFF" }));
    run(&mut cfg,"Configure JUCE WASM")?;
    run(Command::new("cmake").arg("--build").arg(&cmake_dir).arg("--parallel").arg("1"),"Build JUCE WASM")?;
    if !wasm.is_file(){return Err(format!("JUCE build completed but {} is missing",wasm.display()).into());}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_conversion_preserves_other_methods() {
        let s="bool KFuzzAudioProcessor::hasEditor() const { return true; }\njuce::AudioProcessorEditor* KFuzzAudioProcessor::createEditor() { return new Editor(*this); }\nvoid other() {}";
        let s=replace_method_body(s,"KFuzzAudioProcessor","hasEditor","return false;").unwrap();
        let s=replace_method_body(&s,"KFuzzAudioProcessor","createEditor","return nullptr;").unwrap();
        assert!(s.contains("return false;"));assert!(s.contains("return nullptr;"));assert!(s.contains("void other() {}"));
    }

    #[test]
    fn header_only_noisegate_editor_is_removed_without_changing_dsp() {
        // Keep this test independent of the private JUCE regression fixtures.
        let header = r#"
            class NoiseGate final : public AudioProcessor {
              public:
                void processBlock(AudioBuffer<float>&, MidiBuffer&) override { /* DSP */ }
                AudioProcessorEditor* createEditor() override
                { return new GenericAudioProcessorEditor(*this); }
                bool hasEditor() const override { return true; }
            };"#;
        let converted = replace_inline_method_body(header, "hasEditor", "return false;").unwrap();
        let converted = replace_inline_method_body(&converted, "createEditor", "return nullptr;").unwrap();
        assert!(converted.contains("return nullptr;"));
        assert!(converted.contains("return false;"));
        assert!(!converted.contains("new GenericAudioProcessorEditor(*this)"));
        assert!(converted.contains("void processBlock("));
        assert!(header.contains("new GenericAudioProcessorEditor(*this)"));
    }

    #[test]
    fn inline_conversion_refuses_unknown_layout() {
        let header = "struct Example { void createEditor(); bool hasEditor() const; };";
        assert!(replace_inline_method_body(header, "createEditor", "return nullptr;").is_err());
    }
    #[test]
    fn headless_methods_allow_space_before_parentheses() {
        let header = "class Processor { public: bool hasEditor () const override { return true; } \
            juce::AudioProcessorEditor* createEditor () override; };";
        let rewritten = replace_inline_method_body(header, "hasEditor", "return false;").unwrap();
        assert!(rewritten.contains("return false;"));
        assert!(declared_method_in_header(&rewritten, "createEditor"));
    }

    #[test]
    fn mixed_cpp_and_declared_header_methods_generate_only_the_missing_method() {
        let root = std::env::temp_dir().join(format!(
            "wyldtone-mixed-stage-test-{}", std::process::id()
        ));
        let plugin = root.join("plugin");
        let staging = root.join("stage");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(plugin.join("src")).unwrap();
        let original_header = "class NoAmpAudioProcessor : public juce::AudioProcessor {\n\
            public: bool hasEditor() const override;\n\
            juce::AudioProcessorEditor* createEditor() override;\n};";
        let original_cpp = "#include \"PluginProcessor.h\"\n\
            #include \"PluginEditor.h\"\n\
            juce::AudioProcessorEditor* NoAmpAudioProcessor::createEditor() { return new PluginEditor(); }\n\
            void dsp_still_works() {}";
        fs::write(plugin.join("src/PluginProcessor.h"), original_header).unwrap();
        fs::write(plugin.join("src/PluginProcessor.cpp"), original_cpp).unwrap();
        let header_only = stage_source(
            &plugin, &staging, "NoAmpAudioProcessor", "PluginProcessor.h"
        ).unwrap();
        assert!(!header_only);
        assert_eq!(fs::read_to_string(plugin.join("src/PluginProcessor.cpp")).unwrap(), original_cpp);
        let staged_cpp = fs::read_to_string(staging.join("src/PluginProcessor.cpp")).unwrap();
        assert!(staged_cpp.contains("void dsp_still_works() {}"));
        assert!(staged_cpp.contains("return nullptr;"));
        assert!(!staged_cpp.contains("PluginEditor.h"));
        let generated = fs::read_to_string(staging.join("src/WyldtoneHeadlessEditor.cpp")).unwrap();
        assert!(generated.contains("NoAmpAudioProcessor::hasEditor() const"));
        assert!(generated.contains("return false;"));
        assert!(!generated.contains("createEditor()"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mixed_header_inline_and_cpp_definition_are_both_rewritten() {
        let root = std::env::temp_dir().join(format!(
            "wyldtone-inline-stage-test-{}", std::process::id()
        ));
        let plugin = root.join("plugin");
        let staging = root.join("stage");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(plugin.join("src")).unwrap();
        fs::write(plugin.join("src/PluginProcessor.h"),
            "class ExampleProcessor : public juce::AudioProcessor {\n\
             public: bool hasEditor() const override { return true; }\n\
             juce::AudioProcessorEditor* createEditor() override;\n};"
        ).unwrap();
        fs::write(plugin.join("src/PluginProcessor.cpp"),
            "juce::AudioProcessorEditor* ExampleProcessor::createEditor() { return new Editor(); }"
        ).unwrap();
        stage_source(&plugin, &staging, "ExampleProcessor", "PluginProcessor.h").unwrap();
        let header = fs::read_to_string(staging.join("src/PluginProcessor.h")).unwrap();
        assert!(header.contains("return false;"));
        let cpp = fs::read_to_string(staging.join("src/PluginProcessor.cpp")).unwrap();
        assert!(cpp.contains("return nullptr;"));
        assert!(!staging.join("src/WyldtoneHeadlessEditor.cpp").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn recursive_staging_preserves_nested_dsp_sources() {
        let root = std::env::temp_dir().join(format!("wyldtone-stage-test-{}", std::process::id()));
        let plugin = root.join("plugin");
        let staging = root.join("stage");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(plugin.join("src/dsp")).unwrap();
        fs::write(plugin.join("src/PluginProcessor.h"), "class TestProcessor {}; ").unwrap();
        fs::write(plugin.join("src/PluginProcessor.cpp"),
            "bool TestProcessor::hasEditor() const { return true; }\n\
             juce::AudioProcessorEditor* TestProcessor::createEditor() { return new Editor(); }"
        ).unwrap();
        fs::write(plugin.join("src/dsp/Circuit.h"), "// nested DSP header").unwrap();
        fs::write(plugin.join("src/dsp/Circuit.cpp"), "// nested DSP source").unwrap();
        fs::write(plugin.join("src/PluginEditor.cpp"), "// should be skipped").unwrap();
        let result = stage_source(&plugin, &staging, "TestProcessor", "PluginProcessor.h").unwrap();
        assert!(!result);
        assert!(staging.join("src/dsp/Circuit.h").is_file());
        assert!(staging.join("src/dsp/Circuit.cpp").is_file());
        assert!(!staging.join("src/PluginEditor.cpp").exists());
        let processor = fs::read_to_string(staging.join("src/PluginProcessor.cpp")).unwrap();
        assert!(processor.contains("return nullptr;"));
        assert!(processor.contains("return false;"));
        let _ = fs::remove_dir_all(&root);
    }

}
