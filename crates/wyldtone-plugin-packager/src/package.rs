use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use wyldtone_plugin_api::manifest::PluginManifest;

/// A loaded Wyldtone Audio Plugin package.
///
/// The package contents are kept entirely in memory after loading.
/// No files are extracted to disk.
#[derive(Debug)]
pub struct WapPackage {
    pub manifest: PluginManifest,
    pub wasm: Vec<u8>,
}

pub fn create_wap(
    wasm_path: &Path,
    manifest_path: &Path,
    output_path: &Path,
) -> Result<(), Box<dyn Error>> {
    create_wap_with_icon(wasm_path, manifest_path, output_path, None)
}

/// Backwards-compatible WAP extension: optional icon.png or icon.svg (max 64 KiB).
/// No change to the manifest format version or WASM ABI.
pub fn create_wap_with_icon(
    wasm_path: &Path,
    manifest_path: &Path,
    output_path: &Path,
    icon_path: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    if !wasm_path.is_file() {
        return Err(format!(
            "WASM file does not exist: {}",
            wasm_path.display()
        )
        .into());
    }

    if !manifest_path.is_file() {
        return Err(format!(
            "Manifest file does not exist: {}",
            manifest_path.display()
        )
        .into());
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(output_path)?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default();

    let mut wasm = File::open(wasm_path)?;
    let mut wasm_data = Vec::new();
    wasm.read_to_end(&mut wasm_data)?;

    zip.start_file("plugin.wasm", options)?;
    zip.write_all(&wasm_data)?;

    let mut manifest = File::open(manifest_path)?;
    let mut manifest_data = Vec::new();
    manifest.read_to_end(&mut manifest_data)?;

    zip.start_file("manifest.json", options)?;
    zip.write_all(&manifest_data)?;

    if let Some(path) = icon_path {
        let icon = std::fs::read(path)?;
        let entry_name = match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("svg") => {
                validate_svg_icon(&icon)?;
                "icon.svg"
            }
            Some(ext) if ext.eq_ignore_ascii_case("png") => {
                validate_png_icon(&icon)?;
                "icon.png"
            }
            _ => return Err("Plugin icon must be .svg or .png".into()),
        };
        zip.start_file(entry_name, options)?;
        zip.write_all(&icon)?;
    }

    zip.finish()?;

    Ok(())
}

/// Keep icon decoding out of the real-time thread and avoid huge/untrusted assets.
/// A malformed icon must never be served to the UI as trusted markup.
pub const MAX_PLUGIN_ICON_BYTES: usize = 64 * 1024;

pub fn validate_png_icon(icon: &[u8]) -> Result<(), Box<dyn Error>> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if icon.len() < 24 || icon.len() > MAX_PLUGIN_ICON_BYTES
        || !icon.starts_with(PNG_SIGNATURE)
        || &icon[12..16] != b"IHDR"
    {
        return Err("Plugin icon must be PNG with IHDR, at most 64 KiB".into());
    }
    let width = u32::from_be_bytes(icon[16..20].try_into()?);
    let height = u32::from_be_bytes(icon[20..24].try_into()?);
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return Err("Plugin icon dimensions must be 1..=512 pixels".into());
    }
    Ok(())
}

/// UI artwork, always decoded and validated outside the real-time audio callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconFormat { Svg, Png }

#[derive(Debug, PartialEq, Eq)]
pub struct WapIcon {
    pub format: IconFormat,
    pub bytes: Vec<u8>,
}

/// Strict fail-closed subset of SVG for plugin icons.
/// No scripts, CSS, links, external resources, entities, embedded HTML, filters,
/// animation, or image elements. The SVG is used only as a CSS alpha mask.
pub fn validate_svg_icon(data: &[u8]) -> Result<(), Box<dyn Error>> {
    if data.is_empty() || data.len() > MAX_PLUGIN_ICON_BYTES {
        return Err("WAP icon.svg must be 1..=64 KiB".into());
    }
    let xml = std::str::from_utf8(data)?.trim_start_matches('\u{feff}').trim();
    let mut remaining = xml;
    let mut stack: Vec<&str> = Vec::new();
    let mut element_count = 0usize;
    let mut root_done = false;
    while !remaining.is_empty() {
        if !remaining.starts_with('<') {
            let next = remaining.find('<').unwrap_or(remaining.len());
            if !remaining[..next].trim().is_empty() {
                return Err("SVG icon may not contain text nodes".into());
            }
            remaining = &remaining[next..];
            continue;
        }
        if remaining.starts_with("<!") || remaining.starts_with("<?") {
            return Err("SVG declarations, entities and processing instructions are forbidden".into());
        }
        // Locate the end of the tag without accepting '>' inside attributes.
        let mut quote = None;
        let mut end = None;
        for (i, c) in remaining.char_indices().skip(1) {
            if let Some(q) = quote {
                if c == q { quote = None; }
            } else if c == '\'' || c == '"' { quote = Some(c); }
            else if c == '>' { end = Some(i); break; }
        }
        let i = end.ok_or("Unterminated SVG element")?;
        let raw = remaining[1..i].trim();
        remaining = &remaining[i + 1..];
        if let Some(closing) = raw.strip_prefix('/') {
            let name = closing.trim();
            if stack.pop() != Some(name) { return Err("Mismatched SVG closing tag".into()); }
            if stack.is_empty() { root_done = true; }
            continue;
        }
        if root_done { return Err("SVG must have exactly one root".into()); }
        let body = raw.strip_suffix('/').unwrap_or(raw).trim();
        let name_len = body.find(char::is_whitespace).unwrap_or(body.len());
        let name = &body[..name_len];
        const ELEMENTS: &[&str] = &["svg", "g", "path", "circle", "rect", "ellipse", "line", "polyline", "polygon"];
        if !ELEMENTS.contains(&name) || (element_count == 0 && name != "svg") || (element_count > 0 && name == "svg") {
            return Err("Unsupported or unsafe SVG element".into());
        }
        element_count += 1;
        if element_count > 120 || stack.len() >= 12 { return Err("SVG icon is too complex".into()); }
        let mut attrs = body[name_len..].trim();
        let mut has_viewbox = false;
        let mut has_xmlns = false;
        while !attrs.is_empty() {
            let n = attrs.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(attrs.len());
            if n == 0 { return Err("Invalid SVG attribute name".into()); }
            let key = &attrs[..n];
            attrs = attrs[n..].trim_start();
            if !attrs.starts_with('=') { return Err("SVG attributes must have values".into()); }
            attrs = attrs[1..].trim_start();
            let delimiter = attrs.chars().next().ok_or("Missing SVG attribute value")?;
            if delimiter != '"' && delimiter != '\'' { return Err("SVG attributes must be quoted".into()); }
            let tail = &attrs[1..];
            let value_end = tail.find(delimiter).ok_or("Unterminated SVG attribute value")?;
            let value = &tail[..value_end];
            attrs = tail[value_end + 1..].trim_start();
            if key == "viewBox" { has_viewbox = true; }
            if key == "xmlns" { has_xmlns = true; }
            if !safe_svg_attribute(key, value) {
                return Err(format!("Unsafe SVG attribute or value: {key}").into());
            }
        }
        if element_count == 1 && (!has_viewbox || !has_xmlns) {
            return Err("SVG icon requires an SVG namespace and viewBox".into());
        }
        if !raw.ends_with('/') { stack.push(name); }
        else if stack.is_empty() { root_done = true; }
    }
    if !stack.is_empty() || !root_done || element_count < 2 { return Err("Incomplete SVG icon".into()); }
    Ok(())
}

fn safe_svg_attribute(name: &str, value: &str) -> bool {
    if value.len() > 8192 || value.contains('&') || value.contains('<') || value.contains('>') { return false; }
    match name {
        "xmlns" => value == "http://www.w3.org/2000/svg",
        "fill" | "stroke" => value == "none" || value == "currentColor"
            || matches!(value, "black" | "white")
            || (value.starts_with('#') && matches!(value.len(), 4 | 7 | 9)
                && value[1..].bytes().all(|b| b.is_ascii_hexdigit())),
        "stroke-linecap" => matches!(value, "round" | "square" | "butt"),
        "stroke-linejoin" => matches!(value, "round" | "bevel" | "miter"),
        // Standard monochrome icon attributes, e.g. the dashed detector path
        // of the Smooth Downward Expander. Only finite numeric dash patterns
        // are permitted; CSS, URLs and references remain forbidden.
        "stroke-dasharray" => value == "none" || safe_svg_dasharray(value),
        "stroke-dashoffset" => value.parse::<f32>().is_ok_and(|n| n.is_finite()),
        "fill-rule" | "clip-rule" => matches!(value, "evenodd" | "nonzero"),
        "viewBox" | "width" | "height" | "stroke-width" | "opacity" |
        "fill-opacity" | "stroke-opacity" | "stroke-miterlimit" |
        "x" | "y" | "x1" | "y1" | "x2" | "y2" | "cx" | "cy" |
        "r" | "rx" | "ry" | "points" =>
            !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()
                || matches!(b, b' ' | b',' | b'.' | b'-' | b'+' | b'e' | b'E')), 
        "d" => !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()
            || matches!(b, b' ' | b',' | b'.' | b'-' | b'+' | b'e' | b'E'
              | b'M' | b'm' | b'L' | b'l' | b'H' | b'h' | b'V' | b'v'
              | b'C' | b'c' | b'S' | b's' | b'Q' | b'q' | b'T' | b't'
              | b'A' | b'a' | b'Z' | b'z')),
        _ => false,
    }
}

/// A dashed stroke contains a comma/whitespace-separated list of lengths.
/// Restrict it to a small number of nonnegative, finite unitless values.
/// In particular, CSS expressions and external resources are not accepted.
fn safe_svg_dasharray(value: &str) -> bool {
    let mut count = 0;
    for part in value.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
    {
        count += 1;
        if count > 64 || !part.parse::<f32>().is_ok_and(|n| n.is_finite() && n >= 0.0) {
            return false;
        }
    }
    count > 0
}

/// Read package artwork in priority order: validated SVG, then validated PNG.
/// The registry may fall back to the built-in effect icon on any validation error.
pub fn load_wap_icon(path: impl AsRef<Path>) -> Result<Option<WapIcon>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    for (name, format) in [("icon.svg", IconFormat::Svg), ("icon.png", IconFormat::Png)] {
        let Ok(mut entry) = archive.by_name(name) else { continue; };
        if entry.size() > MAX_PLUGIN_ICON_BYTES as u64 { return Err("WAP icon exceeds 64 KiB".into()); }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        match format {
            IconFormat::Svg => validate_svg_icon(&bytes)?,
            IconFormat::Png => validate_png_icon(&bytes)?,
        }
        return Ok(Some(WapIcon { format, bytes }));
    }
    Ok(None)
}

/// Backward-compatible helper for consumers that explicitly need a PNG icon.
pub fn load_wap_icon_png(path: impl AsRef<Path>) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let Ok(mut entry) = archive.by_name("icon.png") else { return Ok(None); };
    if entry.size() > MAX_PLUGIN_ICON_BYTES as u64 { return Err("WAP icon.png exceeds 64 KiB".into()); }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    validate_png_icon(&bytes)?;
    Ok(Some(bytes))
}

/// Load a `.wap` package from disk.
///
/// The package is a ZIP archive containing:
///
/// - `manifest.json`
/// - `plugin.wasm`
///
/// Both files are read directly into memory. Nothing is extracted
/// to the filesystem.
pub fn load_wap(
    path: impl AsRef<Path>,
) -> Result<WapPackage, Box<dyn Error>> {
    let path = path.as_ref();

    if !path.is_file() {
        return Err(format!(
            "WAP file does not exist: {}",
            path.display()
        )
        .into());
    }

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let manifest_data = {
        let mut manifest_file =
            archive
                .by_name("manifest.json")
                .map_err(|_| {
                    "WAP package does not contain `manifest.json`"
                })?;

        if manifest_file.size() > 1024 * 1024 {
            return Err("WAP manifest.json exceeds 1 MiB".into());
        }
        let mut data = Vec::new();
        manifest_file.read_to_end(&mut data)?;
        data
    };

    let manifest: PluginManifest =
        serde_json::from_slice(&manifest_data)?;

    let wasm = {
        let mut wasm_file =
            archive
                .by_name("plugin.wasm")
                .map_err(|_| {
                    "WAP package does not contain `plugin.wasm`"
                })?;

        if wasm_file.size() > 64 * 1024 * 1024 {
            return Err("WAP plugin.wasm exceeds 64 MiB".into());
        }
        let mut data = Vec::new();
        wasm_file.read_to_end(&mut data)?;
        data
    };

    if wasm.is_empty() {
        return Err(
            "WAP package contains an empty `plugin.wasm`"
                .into(),
        );
    }

    Ok(WapPackage {
        manifest,
        wasm,
    })
}

pub fn load_wap_manifest(path: impl AsRef<Path>) -> Result<PluginManifest, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut manifest_file = archive.by_name("manifest.json")?;

    let mut json = String::new();
    manifest_file.read_to_string(&mut json)?;

    let manifest = serde_json::from_str(&json)?;

    Ok(manifest)
}

pub fn package_name(plugin_name: &str) -> String {
    let mut result = String::new();

    for character in plugin_name.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character);
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }

    result = result.trim_matches('-').to_string();

    if result.is_empty() {
        result = "plugin".to_string();
    }

    result.push_str(".wap");

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use wyldtone_plugin_api::manifest::{
        ManifestParameterDefault,
        ManifestParameterType,
        PluginManifest,
        PluginMetadata,
        MANIFEST_FORMAT_VERSION,
    };

    use std::io::Cursor;

    #[test]
    fn monochrome_svg_icons_are_accepted_and_active_content_is_rejected() {
        let valid = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M2 12 L8 6 L16 18 L22 12"/></svg>"##;
        assert!(validate_svg_icon(valid).is_ok());
        for invalid in [
            &br##"<svg viewBox="0 0 24 24"><script>alert(1)</script></svg>"##[..],
            &br##"<svg viewBox="0 0 24 24"><image href="https://example.com/x.svg"/></svg>"##[..],
            &br##"<svg viewBox="0 0 24 24"><path d="M0 0" onload="evil()"/></svg>"##[..],
            &br##"<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><svg/>"##[..],
            &br##"<svg viewBox="0 0 24 24"><foreignObject/></svg>"##[..],
            &br##"<svg viewBox="0 0 24 24"><path d="M0 0" style="color:red"/></svg>"##[..],
            &br##"<svg viewBox="0 0 24 24"><path d="M0 0" stroke="url(#remote)"/></svg>"##[..],
        ] {
            assert!(validate_svg_icon(invalid).is_err(), "unsafe SVG was accepted");
        }
    }

    #[test]
    fn expander_svg_with_dashed_stroke_is_accepted() {
        // Exact content of the icon shipped with Smooth Downward Expander v29.
        let icon = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 19h18M4 17l5-5 4-2 7-6"/>
  <path d="M3 7h6l4 3h8" stroke-dasharray="1 3"/>
  <path d="M13 10l2 2m-2-2 2-2"/>
</svg>"##;
        assert!(validate_svg_icon(icon).is_ok());
    }

    #[test]
    fn dashed_strokes_reject_unsafe_values() {
        assert!(safe_svg_attribute("stroke-dasharray", "1 3"));
        assert!(safe_svg_attribute("stroke-dasharray", "1, 3.5 2"));
        assert!(safe_svg_attribute("stroke-dasharray", "none"));
        assert!(safe_svg_attribute("stroke-dashoffset", "-0.5"));
        for bad in ["url(https://example.com/1)", "1;onload=evil()", "1 -2", "NaN", "Infinity", "", "1em 2px"] {
            assert!(!safe_svg_attribute("stroke-dasharray", bad), "unexpectedly allowed: {bad}");
        }
        assert!(!safe_svg_attribute("stroke-dashoffset", "url(#evil)"));
    }

    #[test]
    fn wap_icon_prefers_svg_and_keeps_png_support() {
        let mut data = Cursor::new(Vec::new());
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M2 2 L22 22"/></svg>"##;
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&[0, 0, 0, 13]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&24u32.to_be_bytes());
        png.extend_from_slice(&24u32.to_be_bytes());
        {
            let mut zip = ZipWriter::new(&mut data);
            let opts = SimpleFileOptions::default();
            zip.start_file("icon.png", opts).unwrap();
            zip.write_all(&png).unwrap();
            zip.start_file("icon.svg", opts).unwrap();
            zip.write_all(svg).unwrap();
            zip.finish().unwrap();
        }
        let tmp = std::env::temp_dir().join(format!("wyldtone-svg-package-{}.wap", std::process::id()));
        std::fs::write(&tmp, data.into_inner()).unwrap();
        assert_eq!(load_wap_icon(&tmp).unwrap(), Some(WapIcon {
            format: IconFormat::Svg, bytes: svg.to_vec(),
        }));
        assert_eq!(load_wap_icon_png(&tmp).unwrap(), Some(png));
        std::fs::remove_file(&tmp).unwrap();
    }

    #[test]
    fn plugin_icon_rejects_oversized_and_invalid_content() {
        assert!(validate_png_icon(&vec![0; MAX_PLUGIN_ICON_BYTES + 1]).is_err());
        assert!(validate_png_icon(b"<svg>untrusted</svg>").is_err());
    }

    fn test_manifest() -> PluginManifest {
        PluginManifest {
            format_version: MANIFEST_FORMAT_VERSION,

            plugin: PluginMetadata {
                id: "com.example.test".to_string(),
                name: "Test Plugin".to_string(),
                manufacturer: "Example".to_string(),
                version: "1.0.0".to_string(),
            category: None,
            },

            parameters: vec![
                wyldtone_plugin_api::manifest::ManifestParameter {
                    id: "drive".to_string(),
                    name: "Drive".to_string(),
                    label: "%".to_string(),
                    parameter_type:
                        ManifestParameterType::Float,
                    min: Some(0.0),
                    max: Some(10.0),
                    default:
                        ManifestParameterDefault::Float(
                            2.0,
                        ),
                    step: Some(0.1),
                    automatable: true,
                    choices: None,
                },
            ],
        }
    }

    fn create_test_wap(
        include_manifest: bool,
        include_wasm: bool,
    ) -> Vec<u8> {
        let mut data =
            Cursor::new(Vec::new());

        {
            let mut zip =
                ZipWriter::new(&mut data);

            let options =
                SimpleFileOptions::default();

            if include_manifest {
                let manifest =
                    serde_json::to_vec(
                        &test_manifest(),
                    )
                    .unwrap();

                zip.start_file(
                    "manifest.json",
                    options,
                )
                .unwrap();

                zip.write_all(&manifest)
                    .unwrap();
            }

            if include_wasm {
                zip.start_file(
                    "plugin.wasm",
                    options,
                )
                .unwrap();

                zip.write_all(
                    b"\0asm\x01\0\0\0",
                )
                .unwrap();
            }

            zip.finish().unwrap();
        }

        data.into_inner()
    }

    #[test]
    fn wap_package_loads_from_zip() {
        let data =
            create_test_wap(true, true);

        let temp_dir =
            std::env::temp_dir();

        let path =
            temp_dir.join(
                "wyldtone-test-package.wap",
            );

        std::fs::write(
            &path,
            &data,
        )
        .unwrap();

        let package =
            load_wap(&path)
                .unwrap();

        assert_eq!(
            package.manifest.plugin.name,
            "Test Plugin"
        );

        assert_eq!(
            package.manifest.parameters.len(),
            1
        );

        assert_eq!(
            package.wasm,
            b"\0asm\x01\0\0\0"
        );

        std::fs::remove_file(
            &path,
        )
        .unwrap();
    }

    #[test]
    fn wap_package_requires_manifest() {
        let data =
            create_test_wap(false, true);

        let path =
            std::env::temp_dir()
                .join(
                    "wyldtone-test-no-manifest.wap",
                );

        std::fs::write(
            &path,
            &data,
        )
        .unwrap();

        let result =
            load_wap(&path);

        assert!(
            result.is_err()
        );

        std::fs::remove_file(
            &path,
        )
        .unwrap();
    }

    #[test]
    fn wap_package_requires_wasm() {
        let data =
            create_test_wap(true, false);

        let path =
            std::env::temp_dir()
                .join(
                    "wyldtone-test-no-wasm.wap",
                );

        std::fs::write(
            &path,
            &data,
        )
        .unwrap();

        let result =
            load_wap(&path);

        assert!(
            result.is_err()
        );

        std::fs::remove_file(
            &path,
        )
        .unwrap();
    }

    #[test]
    fn wap_package_rejects_empty_wasm() {
        let mut data =
            Cursor::new(Vec::new());

        {
            let mut zip =
                ZipWriter::new(&mut data);

            let options =
                SimpleFileOptions::default();

            let manifest =
                serde_json::to_vec(
                    &test_manifest(),
                )
                .unwrap();

            zip.start_file(
                "manifest.json",
                options,
            )
            .unwrap();

            zip.write_all(&manifest)
                .unwrap();

            zip.start_file(
                "plugin.wasm",
                options,
            )
            .unwrap();

            zip.finish().unwrap();
        }

        let path =
            std::env::temp_dir()
                .join(
                    "wyldtone-test-empty-wasm.wap",
                );

        std::fs::write(
            &path,
            data.into_inner(),
        )
        .unwrap();

        let result =
            load_wap(&path);

        assert!(
            result.is_err()
        );

        std::fs::remove_file(
            &path,
        )
        .unwrap();
    }
}