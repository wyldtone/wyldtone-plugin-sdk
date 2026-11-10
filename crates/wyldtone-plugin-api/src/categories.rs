//! Shared versioned public taxonomy for WAP plugin categories.
//! categories.json is copied to the Cloud and mirrored by the Engine at release time.
pub const CATEGORY_SPEC_JSON: &str = include_str!("../../../categories.json");
pub const CATEGORIES: &[&str] = &["Amp", "Cabinet", "Drive", "Dynamics", "EQ / Filter", "Modulation", "Delay", "Reverb", "Pitch", "Synth", "Analyzer", "Utility", "Other"];
pub fn normalize_category(value: &str) -> Option<&'static str> {
    let input = value.trim();
    if let Some(category) = CATEGORIES.iter().copied().find(|c| c.eq_ignore_ascii_case(input)) {
        return Some(category);
    }
    match input.to_ascii_lowercase().as_str() {
        "eq" => Some("EQ / Filter"),
        "equalizer" => Some("EQ / Filter"),
        "eq/filter" => Some("EQ / Filter"),
        "distortion" => Some("Drive"),
        "overdrive" => Some("Drive"),
        "fuzz" => Some("Drive"),
        _ => None,
    }
}
pub fn category_error(value: &str) -> String {
    format!("Invalid plugin category '{}'. Available categories: {}", value, CATEGORIES.join(", "))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn taxonomy_matches_versioned_json() {
        let value: serde_json::Value = serde_json::from_str(CATEGORY_SPEC_JSON).unwrap();
        assert_eq!(value["version"], 1);
        let names = value["categories"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(names, CATEGORIES);
        for (alias, canonical) in value["aliases"].as_object().unwrap() {
            assert_eq!(normalize_category(alias), canonical.as_str());
        }
    }
    #[test]
    fn normalizes_aliases_but_rejects_arbitrary_names() {
        assert_eq!(normalize_category("eq"), Some("EQ / Filter"));
        assert_eq!(normalize_category(" eq / filter "), Some("EQ / Filter"));
        assert_eq!(normalize_category("Distortion"), Some("Drive"));
        assert_eq!(normalize_category("Super Heavy Drive"), None);
    }
}
