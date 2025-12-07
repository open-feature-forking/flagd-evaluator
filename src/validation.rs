//! JSON Schema validation for feature flag configurations.
//!
//! This module provides validation of flag configurations against the official
//! flagd JSON schema from https://github.com/open-feature/flagd-schemas.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;

/// The embedded JSON Schema for flag definitions.
///
/// This schema is loaded from the official flagd-schemas repository at build time.
const FLAGS_SCHEMA: &str = include_str!("../schemas/flags.json");

/// The embedded JSON Schema for targeting rules.
///
/// This schema is referenced by the flags schema.
const TARGETING_SCHEMA: &str = include_str!("../schemas/targeting.json");

/// Fallback error JSON when serialization fails.
const VALIDATION_RESULT_FALLBACK: &str = r#"{"valid":false,"errors":[{"path":"","message":"Failed to serialize validation result"}]}"#;

thread_local! {
    /// Thread-local cached compiled schema.
    ///
    /// In WASM environments, there's a single thread, so we use RefCell for
    /// interior mutability without the overhead of multi-threading primitives.
    static COMPILED_SCHEMA: RefCell<Option<jsonschema::Validator>> = const { RefCell::new(None) };
}

/// Represents a validation error with location and message information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationError {
    /// The JSON path where the error occurred (e.g., "/flags/myFlag/state")
    pub path: String,
    /// A human-readable description of the validation error
    pub message: String,
}

impl ValidationError {
    /// Creates a new validation error.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Represents the result of schema validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the validation succeeded
    pub valid: bool,
    /// List of validation errors (empty if valid)
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Creates a successful validation result.
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    /// Creates a failed validation result with errors.
    pub fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }

    /// Converts the validation result to a JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| VALIDATION_RESULT_FALLBACK.to_string())
    }
}

/// Gets or compiles the JSON schema validator.
///
/// The validator is compiled once per thread and cached for subsequent use.
/// In WASM environments (single-threaded), this effectively caches it globally.
fn get_compiled_schema() -> Result<(), String> {
    COMPILED_SCHEMA.with(|schema| {
        let mut schema_ref = schema.borrow_mut();
        
        // If already compiled, return early
        if schema_ref.is_some() {
            return Ok(());
        }
        
        // Parse the schemas
        let schema_value: Value = serde_json::from_str(FLAGS_SCHEMA)
            .map_err(|e| format!("Failed to parse flags schema: {}", e))?;

        let targeting_schema_value: Value = serde_json::from_str(TARGETING_SCHEMA)
            .map_err(|e| format!("Failed to parse targeting schema: {}", e))?;

        // Create a schema with external resources registered
        let targeting_resource = jsonschema::Resource::from_contents(targeting_schema_value);
        
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft7)
            .with_resource("./targeting.json", targeting_resource)
            .build(&schema_value)
            .map_err(|e| format!("Failed to compile schema: {}", e))?;
        
        *schema_ref = Some(validator);
        Ok(())
    })
}

/// Validates using the cached schema.
fn validate_with_schema(config: &Value) -> Result<(), Vec<ValidationError>> {
    COMPILED_SCHEMA.with(|schema| {
        let schema_ref = schema.borrow();
        let validator = schema_ref.as_ref()
            .ok_or_else(|| vec![ValidationError::new("", "Schema not initialized")])?;
        
        if validator.is_valid(config) {
            Ok(())
        } else {
            let errors: Vec<ValidationError> = validator
                .iter_errors(config)
                .map(|e| ValidationError::new(e.instance_path().to_string(), e.to_string()))
                .collect();
            Err(errors)
        }
    })
}

/// Validates a JSON configuration string against the flagd schema.
///
/// # Arguments
///
/// * `json_str` - The JSON configuration string to validate
///
/// # Returns
///
/// Returns `Ok(())` if validation succeeds, or `Err(ValidationResult)` with detailed
/// error information if validation fails.
///
/// # Example
///
/// ```
/// use flagd_evaluator::validation::validate_flags_config;
///
/// let config = r#"{
///     "flags": {
///         "myFlag": {
///             "state": "ENABLED",
///             "variants": {"on": true, "off": false},
///             "defaultVariant": "on"
///         }
///     }
/// }"#;
///
/// let result = validate_flags_config(config);
/// assert!(result.is_ok());
/// ```
pub fn validate_flags_config(json_str: &str) -> Result<(), ValidationResult> {
    // First, try to parse the JSON
    let config: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            let error = ValidationError::new(
                "",
                format!("Invalid JSON: {}", e),
            );
            return Err(ValidationResult::failure(vec![error]));
        }
    };

    // Ensure the schema is compiled (cached after first use)
    if let Err(e) = get_compiled_schema() {
        let error = ValidationError::new("", e);
        return Err(ValidationResult::failure(vec![error]));
    }

    // Validate the configuration using the cached schema
    match validate_with_schema(&config) {
        Ok(()) => Ok(()),
        Err(errors) => Err(ValidationResult::failure(errors)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_configuration() {
        let config = r#"{
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        if let Err(ref e) = result {
            eprintln!("Validation failed: {}", e.to_json_string());
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_required_fields() {
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "ENABLED"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_err());
        
        let validation_result = result.unwrap_err();
        assert!(!validation_result.valid);
        assert!(!validation_result.errors.is_empty());
    }

    #[test]
    fn test_invalid_state_value() {
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "INVALID_STATE",
                    "variants": {
                        "on": true
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_err());
        
        let validation_result = result.unwrap_err();
        assert!(!validation_result.valid);
        assert!(!validation_result.errors.is_empty());
    }

    #[test]
    fn test_missing_flags_field() {
        let config = r#"{
            "other": "data"
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_err());
        
        let validation_result = result.unwrap_err();
        assert!(!validation_result.valid);
        assert!(!validation_result.errors.is_empty());
    }

    #[test]
    fn test_invalid_json() {
        let config = "not valid json";

        let result = validate_flags_config(config);
        assert!(result.is_err());
        
        let validation_result = result.unwrap_err();
        assert!(!validation_result.valid);
        assert_eq!(validation_result.errors.len(), 1);
        assert!(validation_result.errors[0].message.contains("Invalid JSON"));
    }

    #[test]
    fn test_mixed_variant_types_in_boolean_flag() {
        // Boolean flags should only have boolean variants
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": "false"
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_string_flag() {
        let config = r#"{
            "flags": {
                "colorFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson",
                        "blue": "azure"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_number_flag() {
        let config = r#"{
            "flags": {
                "numFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_object_flag() {
        let config = r#"{
            "flags": {
                "objFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "config1": {"timeout": 30},
                        "config2": {"timeout": 60}
                    },
                    "defaultVariant": "config1"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_flag_with_targeting() {
        let config = r#"{
            "flags": {
                "targetedFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "email"}, "admin@example.com"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_flag_with_metadata() {
        let config = r#"{
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "on",
                    "metadata": {
                        "description": "A test flag"
                    }
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_variants() {
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "ENABLED",
                    "variants": {},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let result = validate_flags_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_result_serialization() {
        let result = ValidationResult::success();
        let json = result.to_json_string();
        assert!(json.contains("\"valid\":true"));

        let errors = vec![
            ValidationError::new("/flags/myFlag", "Missing required field"),
        ];
        let result = ValidationResult::failure(errors);
        let json = result.to_json_string();
        assert!(json.contains("\"valid\":false"));
        assert!(json.contains("Missing required field"));
    }
}
