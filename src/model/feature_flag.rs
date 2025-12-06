//! Feature flag models for flagd JSON schema parsing.
//!
//! This module provides data structures for parsing and working with flagd feature flag
//! configurations as defined in the [flagd specification](https://flagd.dev/reference/flag-definitions/).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a feature flag according to the flagd specification.
///
/// A feature flag contains the state, variants, default variant, optional targeting rules,
/// and optional metadata.
///
/// # Example
///
/// ```
/// use flagd_evaluator::model::FeatureFlag;
/// use serde_json::json;
/// use std::collections::HashMap;
///
/// let flag_json = json!({
///     "state": "ENABLED",
///     "defaultVariant": "on",
///     "variants": {
///         "on": true,
///         "off": false
///     }
/// });
///
/// let flag: FeatureFlag = serde_json::from_value(flag_json).unwrap();
/// assert_eq!(flag.state, "ENABLED");
/// assert_eq!(flag.default_variant, "on");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlag {
    /// The state of the feature flag (e.g., "ENABLED", "DISABLED")
    pub state: String,

    /// The default variant to use when no targeting rule matches
    pub default_variant: String,

    /// Map of variant names to their values (can be any JSON value)
    pub variants: HashMap<String, serde_json::Value>,

    /// Optional targeting rules (JSON Logic expression)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targeting: Option<serde_json::Value>,

    /// Optional metadata associated with the flag
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl FeatureFlag {
    /// Returns the targeting rule as a JSON string.
    ///
    /// If no targeting rule is defined, returns an empty JSON object string "{}".
    ///
    /// # Example
    ///
    /// ```
    /// use flagd_evaluator::model::FeatureFlag;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let mut flag = FeatureFlag {
    ///     state: "ENABLED".to_string(),
    ///     default_variant: "on".to_string(),
    ///     variants: HashMap::new(),
    ///     targeting: Some(json!({"==": [1, 1]})),
    ///     metadata: HashMap::new(),
    /// };
    ///
    /// let targeting_str = flag.get_targeting();
    /// assert!(targeting_str.contains("=="));
    /// ```
    pub fn get_targeting(&self) -> String {
        self.targeting
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "{}".to_string())
    }
}

/// Result of parsing a flagd configuration file.
///
/// Contains the map of feature flags and optional metadata about the flag set.
///
/// # Example
///
/// ```
/// use flagd_evaluator::model::{FeatureFlag, ParsingResult};
/// use serde_json::json;
/// use std::collections::HashMap;
///
/// let config = json!({
///     "flags": {
///         "myFlag": {
///             "state": "ENABLED",
///             "defaultVariant": "on",
///             "variants": {
///                 "on": true,
///                 "off": false
///             }
///         }
///     }
/// });
///
/// let result = ParsingResult::parse(&config.to_string()).unwrap();
/// assert_eq!(result.flags.len(), 1);
/// assert!(result.flags.contains_key("myFlag"));
/// ```
#[derive(Debug, Clone)]
pub struct ParsingResult {
    /// Map of flag names to their FeatureFlag definitions
    pub flags: HashMap<String, FeatureFlag>,

    /// Optional metadata about the flag set
    pub flag_set_metadata: HashMap<String, serde_json::Value>,
}

impl ParsingResult {
    /// Parse a flagd JSON configuration string.
    ///
    /// # Arguments
    ///
    /// * `json_str` - JSON string containing the flagd configuration
    ///
    /// # Returns
    ///
    /// Returns `Ok(ParsingResult)` on success, or an error message on failure.
    ///
    /// # Example
    ///
    /// ```
    /// use flagd_evaluator::model::ParsingResult;
    ///
    /// let config = r#"{
    ///     "flags": {
    ///         "myFlag": {
    ///             "state": "ENABLED",
    ///             "defaultVariant": "on",
    ///             "variants": {
    ///                 "on": true,
    ///                 "off": false
    ///             }
    ///         }
    ///     }
    /// }"#;
    ///
    /// let result = ParsingResult::parse(config).unwrap();
    /// assert_eq!(result.flags.len(), 1);
    /// ```
    pub fn parse(json_str: &str) -> Result<Self, String> {
        // Parse the JSON string
        let config: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        // Extract the flags object
        let flags_obj = config
            .get("flags")
            .ok_or_else(|| "Missing 'flags' field in configuration".to_string())?
            .as_object()
            .ok_or_else(|| "'flags' must be an object".to_string())?;

        // Parse each flag
        let mut flags = HashMap::new();
        for (flag_name, flag_value) in flags_obj {
            let flag: FeatureFlag = serde_json::from_value(flag_value.clone())
                .map_err(|e| format!("Failed to parse flag '{}': {}", flag_name, e))?;
            flags.insert(flag_name.clone(), flag);
        }

        // Extract optional metadata (from root level or other sources)
        let mut flag_set_metadata = HashMap::new();

        // Check for $schema, $evaluators, or other top-level metadata
        if let Some(obj) = config.as_object() {
            for (key, value) in obj {
                if key != "flags" {
                    flag_set_metadata.insert(key.clone(), value.clone());
                }
            }
        }

        Ok(ParsingResult {
            flags,
            flag_set_metadata,
        })
    }

    /// Create an empty ParsingResult.
    pub fn empty() -> Self {
        ParsingResult {
            flags: HashMap::new(),
            flag_set_metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_flag_parsing() {
        let config = r#"{
            "flags": {
                "myBoolFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 1);

        let flag = result.flags.get("myBoolFlag").unwrap();
        assert_eq!(flag.state, "ENABLED");
        assert_eq!(flag.default_variant, "on");
        assert_eq!(flag.variants.len(), 2);
        assert_eq!(flag.variants.get("on"), Some(&json!(true)));
        assert_eq!(flag.variants.get("off"), Some(&json!(false)));
        assert!(flag.targeting.is_none());
    }

    #[test]
    fn test_flag_with_targeting() {
        let config = r#"{
            "flags": {
                "isColorYellow": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {
                                "==": [
                                    {"var": ["color"]},
                                    "yellow"
                                ]
                            },
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        let flag = result.flags.get("isColorYellow").unwrap();

        assert!(flag.targeting.is_some());
        let targeting_str = flag.get_targeting();
        assert!(targeting_str.contains("if"));
        assert!(targeting_str.contains("yellow"));
    }

    #[test]
    fn test_flag_with_metadata() {
        let config = r#"{
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true
                    },
                    "defaultVariant": "on",
                    "metadata": {
                        "description": "A test flag",
                        "version": 1
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        let flag = result.flags.get("myFlag").unwrap();

        assert_eq!(flag.metadata.len(), 2);
        assert_eq!(
            flag.metadata.get("description"),
            Some(&json!("A test flag"))
        );
        assert_eq!(flag.metadata.get("version"), Some(&json!(1)));
    }

    #[test]
    fn test_multiple_flags() {
        let config = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                },
                "flag2": {
                    "state": "DISABLED",
                    "variants": {"off": false},
                    "defaultVariant": "off"
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 2);
        assert!(result.flags.contains_key("flag1"));
        assert!(result.flags.contains_key("flag2"));
    }

    #[test]
    fn test_flag_set_metadata() {
        let config = r#"{
            "$schema": "https://flagd.dev/schema/v0/flags.json",
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            },
            "$evaluators": {
                "emailWithFaas": {
                    "in": ["@faas.com", {"var": ["email"]}]
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 1);

        // Check that metadata includes $schema and $evaluators
        assert!(result.flag_set_metadata.contains_key("$schema"));
        assert!(result.flag_set_metadata.contains_key("$evaluators"));
    }

    #[test]
    fn test_invalid_json() {
        let config = "not valid json";
        let result = ParsingResult::parse(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse JSON"));
    }

    #[test]
    fn test_missing_flags_field() {
        let config = r#"{"other": "data"}"#;
        let result = ParsingResult::parse(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'flags' field"));
    }

    #[test]
    fn test_flags_not_object() {
        let config = r#"{"flags": "not an object"}"#;
        let result = ParsingResult::parse(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("'flags' must be an object"));
    }

    #[test]
    fn test_invalid_flag_structure() {
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "ENABLED"
                }
            }
        }"#;
        let result = ParsingResult::parse(config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Failed to parse flag 'badFlag'"));
    }

    #[test]
    fn test_empty_flags() {
        let config = r#"{"flags": {}}"#;
        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 0);
    }

    #[test]
    fn test_get_targeting_with_rule() {
        let flag = FeatureFlag {
            state: "ENABLED".to_string(),
            default_variant: "on".to_string(),
            variants: HashMap::new(),
            targeting: Some(json!({"==": [1, 1]})),
            metadata: HashMap::new(),
        };

        let targeting = flag.get_targeting();
        assert!(targeting.contains("=="));
        assert_ne!(targeting, "{}");
    }

    #[test]
    fn test_get_targeting_without_rule() {
        let flag = FeatureFlag {
            state: "ENABLED".to_string(),
            default_variant: "on".to_string(),
            variants: HashMap::new(),
            targeting: None,
            metadata: HashMap::new(),
        };

        let targeting = flag.get_targeting();
        assert_eq!(targeting, "{}");
    }

    #[test]
    fn test_flag_with_different_variant_types() {
        let config = r#"{
            "flags": {
                "multiTypeFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "string": "value",
                        "number": 42,
                        "float": 3.14,
                        "bool": true,
                        "object": {"key": "val"},
                        "array": [1, 2, 3]
                    },
                    "defaultVariant": "string"
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        let flag = result.flags.get("multiTypeFlag").unwrap();

        assert_eq!(flag.variants.len(), 6);
        assert_eq!(flag.variants.get("string"), Some(&json!("value")));
        assert_eq!(flag.variants.get("number"), Some(&json!(42)));
        assert_eq!(flag.variants.get("bool"), Some(&json!(true)));
    }

    #[test]
    fn test_empty_parsing_result() {
        let result = ParsingResult::empty();
        assert_eq!(result.flags.len(), 0);
        assert_eq!(result.flag_set_metadata.len(), 0);
    }

    #[test]
    fn test_flag_equality() {
        let flag1 = FeatureFlag {
            state: "ENABLED".to_string(),
            default_variant: "on".to_string(),
            variants: HashMap::new(),
            targeting: None,
            metadata: HashMap::new(),
        };

        let flag2 = FeatureFlag {
            state: "ENABLED".to_string(),
            default_variant: "on".to_string(),
            variants: HashMap::new(),
            targeting: None,
            metadata: HashMap::new(),
        };

        assert_eq!(flag1, flag2);
    }

    #[test]
    fn test_flag_serialization() {
        let mut variants = HashMap::new();
        variants.insert("on".to_string(), json!(true));
        variants.insert("off".to_string(), json!(false));

        let flag = FeatureFlag {
            state: "ENABLED".to_string(),
            default_variant: "on".to_string(),
            variants,
            targeting: Some(json!({"==": [1, 1]})),
            metadata: HashMap::new(),
        };

        let serialized = serde_json::to_string(&flag).unwrap();
        let deserialized: FeatureFlag = serde_json::from_str(&serialized).unwrap();

        assert_eq!(flag, deserialized);
    }

    #[test]
    fn test_flag_deserialization_with_camel_case() {
        let json = r#"{
            "state": "ENABLED",
            "defaultVariant": "on",
            "variants": {"on": true},
            "targeting": {"==": [1, 1]},
            "metadata": {"key": "value"}
        }"#;

        let flag: FeatureFlag = serde_json::from_str(json).unwrap();
        assert_eq!(flag.state, "ENABLED");
        assert_eq!(flag.default_variant, "on");
        assert!(flag.targeting.is_some());
        assert_eq!(flag.metadata.get("key"), Some(&json!("value")));
    }
}
