use serde::{Deserialize, Serialize};

use crate::config::PluginConfig;

use crate::parameter::{
    Parameter,
    ParameterKind,
    ParameterValue,
};

pub const MANIFEST_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub format_version: u32,
    pub plugin: PluginMetadata,
    pub parameters: Vec<ManifestParameter>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional OSS license metadata; unrelated to future entitlement/DRM licensing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<PluginLicense>,
}

/// Attribution metadata that stays readable inside the WAP manifest.
/// Full legal notices are also included as ZIP entries at the stated paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLicense {
    /// SPDX expression e.g. "Apache-2.0 AND MIT".
    pub expression: String,
    /// Entries inside the WAP ZIP, not external links or filesystem paths.
    pub notices: Vec<LicenseNotice>,
    /// HTTPS link to the complete corresponding source of THIS released WAP,
    /// ideally a fixed commit/tag or immutable source archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// HTTPS link to matching build/install instructions and required scripts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_instructions_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseNotice {
    pub component: String,
    pub spdx: String,
    pub path: String,
}

/// Only HTTPS references without credentials or dangerous/ambiguous URL text
/// are accepted. Links are metadata, never downloaded by the SDK.
fn valid_public_source_url(link: &str) -> bool {
    if link.len() > 2048 || !link.starts_with("https://") {
        return false;
    }
    if link.bytes().any(|byte|
        byte <= 0x20 || byte >= 0x7f
        || matches!(byte, b'\\' | b'<' | b'>' | b'"' | b'\'' | b'`')
    ) {
        return false;
    }
    let rest = &link["https://".len()..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') || authority.contains('%')
        || authority.contains(':') || authority.contains('[') || authority.contains(']')
        || !authority.contains('.')
        || authority.starts_with('.') || authority.ends_with('.')
        || !authority.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-'))
    {
        return false;
    }
    true
}

impl PluginLicense {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_url.as_deref().is_some_and(|url| !valid_public_source_url(url))
            || self.build_instructions_url.as_deref().is_some_and(|url| !valid_public_source_url(url))
        {
            return Err("Source and build instruction URLs must be valid public HTTPS links".into());
        }
        if self.expression.is_empty() || self.expression.len() > 128
            || !self.expression.bytes().all(|b| b.is_ascii_alphanumeric()
                || matches!(b, b' ' | b'.' | b'-' | b'+' | b'(' | b')'))
        {
            return Err("Invalid SPDX license expression".into());
        }
        if self.notices.is_empty() || self.notices.len() > 16 {
            return Err("License metadata must contain 1..=16 notices".into());
        }
        let mut paths = std::collections::HashSet::new();
        for notice in &self.notices {
            let path = &notice.path;
            let Some(name) = path.strip_prefix("licenses/") else {
                return Err("License notices must be under licenses/".into());
            };
            if name.is_empty() || name.len() > 100 || name.contains("..")
                || !name.bytes().all(|b| b.is_ascii_alphanumeric()
                    || matches!(b, b'-' | b'_' | b'.'))
                || !(name.ends_with(".txt") || name.ends_with(".md"))
                || !paths.insert(path)
            {
                return Err(format!("Invalid or duplicate license notice path: {path}"));
            }
            if notice.component.trim().is_empty() || notice.component.len() > 128
                || notice.spdx.is_empty() || notice.spdx.len() > 80
            {
                return Err("License notice needs component and SPDX identifier".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestParameter {
    pub id: String,
    pub name: String,
    pub label: String,

    #[serde(rename = "type")]
    pub parameter_type: ManifestParameterType,

    pub min: Option<f32>,
    pub max: Option<f32>,
    pub default: ManifestParameterDefault,
    pub step: Option<f32>,
    pub automatable: bool,

    pub choices: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestParameterType {
    Float,
    Int,
    Bool,
    Choice,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum ManifestParameterDefault {
    Float(f32),
    Int(i32),
    Bool(bool),
    Choice(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManifestValidationError {
    InvalidParameter {
        parameter_id: String,
    },

    MissingParameter {
        parameter_id: String,
    },

    UnexpectedParameter {
        parameter_id: String,
    },

    ParameterMismatch {
        parameter_id: String,
        field: &'static str,
    },
}

impl std::fmt::Display for ManifestValidationError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::InvalidParameter { parameter_id } => {
                write!(
                    f,
                    "invalid WASM parameter: {parameter_id}"
                )
            }

            Self::MissingParameter { parameter_id } => {
                write!(
                    f,
                    "manifest parameter missing from WASM: {parameter_id}"
                )
            }

            Self::UnexpectedParameter { parameter_id } => {
                write!(
                    f,
                    "WASM parameter missing from manifest: {parameter_id}"
                )
            }

            Self::ParameterMismatch {
                parameter_id,
                field,
            } => {
                write!(
                    f,
                    "parameter '{parameter_id}' differs in field '{field}'"
                )
            }
        }
    }
}

impl std::error::Error for ManifestValidationError {}

impl From<&Parameter> for ManifestParameter {
    fn from(parameter: &Parameter) -> Self {
        let (
            parameter_type,
            min,
            max,
            default,
            step,
            choices,
        ) = match (&parameter.kind, &parameter.default) {
            (
                ParameterKind::Float { min, max, step },
                ParameterValue::Float(default),
            ) => (
                ManifestParameterType::Float,
                Some(*min),
                Some(*max),
                ManifestParameterDefault::Float(*default),
                *step,
                None,
            ),

            (
                ParameterKind::Int { min, max, step },
                ParameterValue::Int(default),
            ) => (
                ManifestParameterType::Int,
                Some(*min as f32),
                Some(*max as f32),
                ManifestParameterDefault::Int(*default),
                Some(*step as f32),
                None,
            ),

            (
                ParameterKind::Bool,
                ParameterValue::Bool(default),
            ) => (
                ManifestParameterType::Bool,
                None,
                None,
                ManifestParameterDefault::Bool(*default),
                None,
                None,
            ),

            (
                ParameterKind::Choice { choices },
                ParameterValue::Choice(default),
            ) => (
                ManifestParameterType::Choice,
                None,
                None,
                ManifestParameterDefault::Choice(*default),
                None,
                Some(choices.clone()),
            ),

            // Parameters are validated when loaded from WASM.
            // Keeping this branch defensive prevents the conversion
            // from silently producing an invalid manifest.
            _ => {
                panic!(
                    "cannot convert invalid parameter '{}' to manifest",
                    parameter.id
                );
            }
        };

        Self {
            id: parameter.id.clone(),
            name: parameter.name.clone(),
            label: parameter.label.clone(),
            parameter_type,
            min,
            max,
            default,
            step,
            automatable: parameter.automatable,
            choices,
        }
    }
}

impl PluginManifest {
    pub fn from_config_and_parameters(
        config: &PluginConfig,
        parameters: &[Parameter],
    ) -> Self {
        Self {
            format_version: MANIFEST_FORMAT_VERSION,

            plugin: PluginMetadata {
                id: config.id.clone(),
                name: config.name.clone(),
                manufacturer: config.manufacturer.clone(),
                version: config.version.clone(),
                category: config.category.clone(),
                description: config.description.clone(),
                license: config.license.clone(),
            },

            parameters: parameters
                .iter()
                .map(ManifestParameter::from)
                .collect(),
        }
    }

    pub fn validate_against(
        &self,
        parameters: &[Parameter],
    ) -> Result<(), ManifestValidationError> {
        for parameter in parameters {
            if !parameter.is_valid() {
                return Err(
                    ManifestValidationError::InvalidParameter {
                        parameter_id: parameter.id.clone(),
                    }
                );
            }

            let manifest_parameter = self
                .parameters
                .iter()
                .find(|manifest| manifest.id == parameter.id);

            let Some(manifest_parameter) = manifest_parameter else {
                return Err(
                    ManifestValidationError::MissingParameter {
                        parameter_id: parameter.id.clone(),
                    }
                );
            };

            let expected = ManifestParameter::from(parameter);

            if manifest_parameter != &expected {
                let field = parameter_mismatch_field(
                    manifest_parameter,
                    &expected,
                );

                return Err(
                    ManifestValidationError::ParameterMismatch {
                        parameter_id: parameter.id.clone(),
                        field,
                    }
                );
            }
        }

        for manifest_parameter in &self.parameters {
            if !parameters
                .iter()
                .any(|parameter| parameter.id == manifest_parameter.id)
            {
                return Err(
                    ManifestValidationError::UnexpectedParameter {
                        parameter_id: manifest_parameter.id.clone(),
                    }
                );
            }
        }

        Ok(())
    }
}

fn parameter_mismatch_field(
    actual: &ManifestParameter,
    expected: &ManifestParameter,
) -> &'static str {
    if actual.name != expected.name {
        return "name";
    }

    if actual.label != expected.label {
        return "label";
    }

    if actual.parameter_type != expected.parameter_type {
        return "type";
    }

    if actual.min != expected.min {
        return "min";
    }

    if actual.max != expected.max {
        return "max";
    }

    if actual.default != expected.default {
        return "default";
    }

    if actual.step != expected.step {
        return "step";
    }

    if actual.automatable != expected.automatable {
        return "automatable";
    }

    if actual.choices != expected.choices {
        return "choices";
    }

    "unknown"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parameters() -> Vec<Parameter> {
        vec![
            Parameter {
                id: "drive".to_string(),
                name: "Drive".to_string(),
                label: "%".to_string(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 10.0,
                    step: Some(0.1),
                },
                default: ParameterValue::Float(2.0),
                value: ParameterValue::Float(2.0),
                automatable: true,
            },

            Parameter {
                id: "oversampling".to_string(),
                name: "Oversampling".to_string(),
                label: "x".to_string(),
                kind: ParameterKind::Int {
                    min: 1,
                    max: 8,
                    step: 1,
                },
                default: ParameterValue::Int(2),
                value: ParameterValue::Int(2),
                automatable: true,
            },

            Parameter {
                id: "bypass".to_string(),
                name: "Bypass".to_string(),
                label: "".to_string(),
                kind: ParameterKind::Bool,
                default: ParameterValue::Bool(false),
                value: ParameterValue::Bool(false),
                automatable: true,
            },

            Parameter {
                id: "mode".to_string(),
                name: "Mode".to_string(),
                label: "".to_string(),
                kind: ParameterKind::Choice {
                    choices: vec![
                        "Clean".to_string(),
                        "Crunch".to_string(),
                        "High Gain".to_string(),
                    ],
                },
                default: ParameterValue::Choice(1),
                value: ParameterValue::Choice(1),
                automatable: true,
            },
        ]
    }

    #[test]
    fn parameters_convert_to_manifest_parameters() {
        let parameters = test_parameters();

        let manifest_parameters: Vec<ManifestParameter> =
            parameters
                .iter()
                .map(ManifestParameter::from)
                .collect();

        assert_eq!(manifest_parameters.len(), 4);

        assert_eq!(
            manifest_parameters[0],
            ManifestParameter {
                id: "drive".to_string(),
                name: "Drive".to_string(),
                label: "%".to_string(),
                parameter_type: ManifestParameterType::Float,
                min: Some(0.0),
                max: Some(10.0),
                default: ManifestParameterDefault::Float(2.0),
                step: Some(0.1),
                automatable: true,
                choices: None,
            }
        );

        assert_eq!(
            manifest_parameters[3].choices,
            Some(vec![
                "Clean".to_string(),
                "Crunch".to_string(),
                "High Gain".to_string(),
            ])
        );
    }

    #[test]
    fn old_metadata_without_category_is_still_deserializable() {
        let json = r#"{"id":"com.example.old","name":"Old","manufacturer":"Test","version":"1"}"#;
        let metadata: PluginMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.category, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.license, None);
    }

    #[test]
    fn license_metadata_is_optional_and_rejects_unsafe_zip_entries() {
        let old: PluginMetadata = serde_json::from_str(
            r#"{"id":"legacy","name":"Old","manufacturer":"Example","version":"1"}"#,
        ).unwrap();
        assert!(old.license.is_none());

        let license = PluginLicense {
            expression: "Apache-2.0 AND MIT".into(),
            notices: vec![LicenseNotice {
                component: "0xFX amp DSP".into(),
                spdx: "MIT".into(),
                path: "licenses/0xFX-MIT.txt".into(),
            }],
            source_url: None,
            build_instructions_url: None,
        };
        license.validate().unwrap();
        let decoded: PluginLicense =
            serde_json::from_str(&serde_json::to_string(&license).unwrap()).unwrap();
        assert_eq!(license, decoded);
        let mut invalid = license;
        invalid.notices[0].path = "licenses/../untrusted.txt".into();
        assert!(invalid.validate().is_err());
        invalid.notices[0].path = "/etc/passwd".into();
        assert!(invalid.validate().is_err());
        invalid.notices[0].path = "licenses/ok.txt".into();
        invalid.notices.push(invalid.notices[0].clone());
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn gpl_source_and_build_urls_roundtrip_in_manifest_license() {
        let metadata = serde_json::json!({
            "expression": "GPL-3.0-or-later",
            "notices": [{
                "component": "GPL plugin source",
                "spdx": "GPL-3.0-or-later",
                "path": "licenses/GPL.txt"
            }],
            "source_url": "https://github.com/example/plugin/tree/76abcde/src",
            "build_instructions_url": "https://github.com/example/plugin/blob/76abcde/BUILD.md"
        });
        let license: PluginLicense = serde_json::from_value(metadata.clone()).unwrap();
        license.validate().unwrap();
        assert_eq!(serde_json::to_value(&license).unwrap(), metadata);
        assert_eq!(
            license.source_url.as_deref(),
            Some("https://github.com/example/plugin/tree/76abcde/src")
        );
        // Older license declarations without URLs remain deserializable.
        let legacy: PluginLicense = serde_json::from_value(serde_json::json!({
            "expression": "MIT",
            "notices": [{
                "component": "Legacy", "spdx": "MIT",
                "path": "licenses/MIT.txt"
            }]
        })).unwrap();
        assert_eq!(legacy.source_url, None);
        assert_eq!(legacy.build_instructions_url, None);
        assert!(!serde_json::to_string(&legacy).unwrap().contains("source_url"));
    }

    #[test]
    fn unsafe_source_and_build_instruction_links_are_rejected() {
        let mut license: PluginLicense = serde_json::from_value(serde_json::json!({
            "expression": "GPL-3.0-only",
            "notices": [{
                "component": "Source", "spdx": "GPL-3.0-only",
                "path": "licenses/GPL.txt"
            }]
        })).unwrap();
        for bad in [
            "javascript:alert(1)",
            "http://example.org/source",
            "https://example.org@attacker.example/code",
            "https://example.org\\evil.test/",
            "https://example.org/a?b=<script>",
            "https:///no-host",
            "https://example.org/unsafe space",
            "https://127.0.0.1/build",
            "https://example.org:443/source",
        ] {
            license.source_url = Some(bad.to_string());
            assert!(license.validate().is_err(), "accepted dangerous URL: {bad}");
        }
        license.source_url = Some("https://github.com/example/plugin/tree/v1.0".into());
        license.build_instructions_url =
            Some("https://github.com/example/plugin/blob/v1.0/BUILD.md".into());
        license.validate().unwrap();
    }

    #[test]
    fn description_is_optional_and_roundtrips_as_plain_text() {
        let text = "<img src=x onerror=alert(1)> & a chorus";
        let value: PluginMetadata = serde_json::from_value(serde_json::json!({
            "id": "com.example.chorus",
            "name": "Chorus",
            "manufacturer": "Example",
            "version": "1.0",
            "description": text
        })).unwrap();
        assert_eq!(value.description.as_deref(), Some(text));
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: PluginMetadata = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.description.as_deref(), Some(text));
    }

    #[test]
    fn manifest_validates_against_matching_parameters() {
        let parameters = test_parameters();

        let manifest = PluginManifest {
            format_version: MANIFEST_FORMAT_VERSION,
            plugin: PluginMetadata {
                id: "com.example.test".to_string(),
                name: "Test".to_string(),
                manufacturer: "Example".to_string(),
                version: "1.0.0".to_string(),
                category: None,
                description: None,
                license: None,
            },
            parameters: parameters
                .iter()
                .map(ManifestParameter::from)
                .collect(),
        };

        assert_eq!(
            manifest.validate_against(&parameters),
            Ok(())
        );
    }

    #[test]
    fn manifest_rejects_parameter_value_changes() {
        let parameters = test_parameters();

        let mut manifest =
            PluginManifest::from_config_and_parameters(
                &PluginConfig {
                    id: "com.example.test".to_string(),
                    name: "Test".to_string(),
                    manufacturer: "Example".to_string(),
                    version: "1.0.0".to_string(),
                    category: None,
                description: None,
                license: None,
                    icon: None,
                    juce: None,
                    cpp: None,
                    faust: None,
                    audio: None,
                },
                &parameters,
            );

        manifest.parameters[0].max = Some(100.0);

        assert_eq!(
            manifest.validate_against(&parameters),
            Err(
                ManifestValidationError::ParameterMismatch {
                    parameter_id: "drive".to_string(),
                    field: "max",
                }
            )
        );
    }

    #[test]
    fn manifest_rejects_missing_wasm_parameter() {
        let parameters = test_parameters();

        let manifest = PluginManifest {
            format_version: MANIFEST_FORMAT_VERSION,
            plugin: PluginMetadata {
                id: "com.example.test".to_string(),
                name: "Test".to_string(),
                manufacturer: "Example".to_string(),
                version: "1.0.0".to_string(),
                category: None,
                description: None,
                license: None,
            },
            parameters: parameters[1..]
                .iter()
                .map(ManifestParameter::from)
                .collect(),
        };

        assert_eq!(
            manifest.validate_against(&parameters),
            Err(
                ManifestValidationError::MissingParameter {
                    parameter_id: "drive".to_string(),
                }
            )
        );
    }

    #[test]
    fn manifest_rejects_unexpected_manifest_parameter() {
        let parameters = test_parameters();

        let mut manifest =
            PluginManifest::from_config_and_parameters(
                &PluginConfig {
                    id: "com.example.test".to_string(),
                    name: "Test".to_string(),
                    manufacturer: "Example".to_string(),
                    version: "1.0.0".to_string(),
                    category: None,
                description: None,
                license: None,
                    icon: None,
                    juce: None,
                    cpp: None,
                    faust: None,
                    audio: None,
                },
                &parameters,
            );

        manifest.parameters.push(ManifestParameter {
            id: "extra".to_string(),
            name: "Extra".to_string(),
            label: "".to_string(),
            parameter_type: ManifestParameterType::Bool,
            min: None,
            max: None,
            default: ManifestParameterDefault::Bool(false),
            step: None,
            automatable: false,
            choices: None,
        });

        assert_eq!(
            manifest.validate_against(&parameters),
            Err(
                ManifestValidationError::UnexpectedParameter {
                    parameter_id: "extra".to_string(),
                }
            )
        );
    }

    #[test]
    fn float_parameter_manifest_roundtrip() {
        let manifest = PluginManifest {
            format_version: MANIFEST_FORMAT_VERSION,
            plugin: PluginMetadata {
                id: "com.example.test".to_string(),
                name: "Test Plugin".to_string(),
                manufacturer: "Example".to_string(),
                version: "1.0.0".to_string(),
                category: None,
                description: None,
                license: None,
            },
            parameters: vec![
                ManifestParameter {
                    id: "drive".to_string(),
                    name: "Drive".to_string(),
                    label: "%".to_string(),
                    parameter_type: ManifestParameterType::Float,
                    min: Some(0.0),
                    max: Some(10.0),
                    default: ManifestParameterDefault::Float(2.0),
                    step: Some(0.1),
                    automatable: true,
                    choices: None,
                },
            ],
        };

        let json = serde_json::to_string_pretty(&manifest)
            .expect("manifest should serialize");

        let decoded: PluginManifest =
            serde_json::from_str(&json)
                .expect("manifest should deserialize");

        assert_eq!(manifest, decoded);
    }

    #[test]
    fn bool_parameter_manifest_roundtrip() {
        let parameter = ManifestParameter {
            id: "bypass".to_string(),
            name: "Bypass".to_string(),
            label: "".to_string(),
            parameter_type: ManifestParameterType::Bool,
            min: None,
            max: None,
            default: ManifestParameterDefault::Bool(false),
            step: None,
            automatable: true,
            choices: None,
        };

        let json = serde_json::to_string(&parameter)
            .expect("parameter should serialize");

        let decoded: ManifestParameter =
            serde_json::from_str(&json)
                .expect("parameter should deserialize");

        assert_eq!(parameter, decoded);
    }

    #[test]
    fn int_parameter_manifest_roundtrip() {
        let parameter = ManifestParameter {
            id: "oversampling".to_string(),
            name: "Oversampling".to_string(),
            label: "x".to_string(),
            parameter_type: ManifestParameterType::Int,
            min: Some(1.0),
            max: Some(8.0),
            default: ManifestParameterDefault::Int(2),
            step: Some(1.0),
            automatable: true,
            choices: None,
        };

        let json = serde_json::to_string(&parameter)
            .expect("parameter should serialize");

        let decoded: ManifestParameter =
            serde_json::from_str(&json)
                .expect("parameter should deserialize");

        assert_eq!(parameter, decoded);
    }

    #[test]
    fn choice_parameter_manifest_roundtrip() {
        let parameter = ManifestParameter {
            id: "mode".to_string(),
            name: "Mode".to_string(),
            label: "".to_string(),
            parameter_type: ManifestParameterType::Choice,
            min: None,
            max: None,
            default: ManifestParameterDefault::Choice(1),
            step: None,
            automatable: true,
            choices: Some(vec![
                "Clean".to_string(),
                "Crunch".to_string(),
                "High Gain".to_string(),
            ]),
        };

        let json = serde_json::to_string(&parameter)
            .expect("parameter should serialize");

        let decoded: ManifestParameter =
            serde_json::from_str(&json)
                .expect("parameter should deserialize");

        assert_eq!(parameter, decoded);
    }
}