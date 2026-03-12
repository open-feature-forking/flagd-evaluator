//! YAML flag configuration loading with schema validation.
//!
//! This module provides helpers for loading flagd flag configurations
//! from YAML format, converting them to JSON for use with [`crate::evaluator::FlagEvaluator::update_state`].

use serde_json::Value;

/// Parse a YAML flag configuration string into a JSON string.
///
/// This converts a YAML flag configuration (as used in flagd `.yaml` files)
/// into the equivalent JSON string, which can then be passed to
/// [`crate::evaluator::FlagEvaluator::update_state`].
///
/// # Errors
///
/// Returns an error string if:
/// - The input is not valid YAML
/// - The YAML cannot be represented as JSON (e.g., YAML-specific types)
///
/// # Example
///
/// ```
/// use flagd_evaluator::yaml::yaml_to_json;
///
/// let yaml = r#"
/// flags:
///   my-flag:
///     state: ENABLED
///     variants:
///       "on": true
///       "off": false
///     defaultVariant: "on"
/// "#;
///
/// let json = yaml_to_json(yaml).unwrap();
/// assert!(json.contains("my-flag"));
/// ```
pub fn yaml_to_json(yaml_str: &str) -> Result<String, String> {
    let value: Value =
        serde_yaml::from_str(yaml_str).map_err(|e| format!("Failed to parse YAML: {}", e))?;
    serde_json::to_string(&value).map_err(|e| format!("Failed to serialize to JSON: {}", e))
}
