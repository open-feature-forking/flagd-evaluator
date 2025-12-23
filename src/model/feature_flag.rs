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
/// assert_eq!(flag.default_variant, Some("on".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlag {
    /// The key/name of the feature flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// The state of the feature flag (e.g., "ENABLED", "DISABLED")
    pub state: String,

    /// The default variant to use when no targeting rule matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_variant: Option<String>,

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
    ///     key: Some("my_flag".to_string()),
    ///     state: "ENABLED".to_string(),
    ///     default_variant: Option::from("on".to_string()),
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

    /// Checks if this flag is different from another flag.
    ///
    /// Compares all fields of the flag using the derived PartialEq implementation.
    /// This includes state, default variant, variants, targeting rules, and metadata.
    ///
    /// # Arguments
    ///
    /// * `other` - The flag to compare against
    ///
    /// # Example
    ///
    /// ```
    /// use flagd_evaluator::model::FeatureFlag;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let flag1 = FeatureFlag {
    ///     key: Some("test".to_string()),
    ///     state: "ENABLED".to_string(),
    ///     default_variant: Option::from("on".to_string()),
    ///     variants: HashMap::new(),
    ///     targeting: Some(json!({"==": [1, 1]})),
    ///     metadata: HashMap::new(),
    /// };
    ///
    /// let mut flag2 = flag1.clone();
    /// flag2.default_variant = Option::from("off".to_string());
    ///
    /// assert!(flag1.is_different_from(&flag2));
    /// ```
    pub fn is_different_from(&self, other: &FeatureFlag) -> bool {
        self != other
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

        // Extract $evaluators if present
        let evaluators =
            if let Some(eval_obj) = config.get("$evaluators").and_then(|v| v.as_object()) {
                let mut map = HashMap::new();
                for (name, rule) in eval_obj {
                    map.insert(name.clone(), rule.clone());
                }
                map
            } else {
                HashMap::new()
            };

        // Extract the flags object
        let flags_obj = config
            .get("flags")
            .ok_or_else(|| "Missing 'flags' field in configuration".to_string())?
            .as_object()
            .ok_or_else(|| "'flags' must be an object".to_string())?;

        // Parse each flag and set its key
        let mut flags = HashMap::new();
        for (flag_name, flag_value) in flags_obj {
            let mut flag: FeatureFlag = serde_json::from_value(flag_value.clone())
                .map_err(|e| format!("Failed to parse flag '{}': {}", flag_name, e))?;
            // Set the flag key
            flag.key = Some(flag_name.clone());

            // Resolve $ref references in targeting rules if evaluators exist
            if !evaluators.is_empty() && flag.targeting.is_some() {
                let targeting = flag.targeting.take().unwrap();
                let mut visited = std::collections::HashSet::new();
                match Self::resolve_refs(&targeting, &evaluators, &mut visited) {
                    Ok(resolved) => flag.targeting = Some(resolved),
                    Err(e) => {
                        return Err(format!(
                            "Failed to resolve $ref in flag '{}': {}",
                            flag_name, e
                        ))
                    }
                }
            }

            flags.insert(flag_name.clone(), flag);
        }

        // Extract flag-set metadata from top-level "metadata" object
        let mut flag_set_metadata = HashMap::new();

        // Flatten top-level "metadata" object into flag_set_metadata
        if let Some(metadata_value) = config.get("metadata") {
            if let Some(metadata_obj) = metadata_value.as_object() {
                for (key, value) in metadata_obj {
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

    /// Resolves $ref references in a JSON value by replacing them with evaluators.
    ///
    /// This function recursively traverses the JSON structure and replaces any
    /// `{"$ref": "evaluatorName"}` objects with the corresponding evaluator definition
    /// from the evaluators map.
    ///
    /// # Arguments
    /// * `value` - The JSON value to process (typically a targeting rule)
    /// * `evaluators` - Map of evaluator names to their definitions
    /// * `visited` - Set of evaluator names already being resolved (for circular reference detection)
    ///
    /// # Returns
    /// * `Ok(Value)` - The JSON value with all $refs resolved
    /// * `Err(String)` - Error if a $ref points to a non-existent evaluator or circular reference detected
    fn resolve_refs(
        value: &serde_json::Value,
        evaluators: &HashMap<String, serde_json::Value>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<serde_json::Value, String> {
        use serde_json::{Map, Value};

        match value {
            Value::Object(obj) => {
                // Check if this is a $ref object
                if obj.len() == 1 && obj.contains_key("$ref") {
                    if let Some(Value::String(ref_name)) = obj.get("$ref") {
                        // Check for circular references
                        if visited.contains(ref_name) {
                            return Err(format!(
                                "Circular reference detected in evaluator: {}",
                                ref_name
                            ));
                        }

                        // Look up the evaluator
                        let evaluator = evaluators.get(ref_name).ok_or_else(|| {
                            format!("Evaluator '{}' not found in $evaluators", ref_name)
                        })?;

                        // Add to visited set and recurse
                        visited.insert(ref_name.clone());
                        let resolved = Self::resolve_refs(evaluator, evaluators, visited)?;
                        visited.remove(ref_name);

                        return Ok(resolved);
                    }
                }

                // Not a $ref, recursively resolve any nested $refs
                let mut resolved_obj = Map::new();
                for (key, val) in obj {
                    resolved_obj.insert(key.clone(), Self::resolve_refs(val, evaluators, visited)?);
                }
                Ok(Value::Object(resolved_obj))
            }
            Value::Array(arr) => {
                // Recursively resolve $refs in array elements
                let mut resolved_arr = Vec::new();
                for item in arr {
                    resolved_arr.push(Self::resolve_refs(item, evaluators, visited)?);
                }
                Ok(Value::Array(resolved_arr))
            }
            // Primitives don't contain $refs
            _ => Ok(value.clone()),
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
        assert_eq!(flag.default_variant.clone().unwrap(), "on");
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
            "metadata": {
                "environment": "production",
                "version": 2
            },
            "$evaluators": {
                "emailWithFaas": {
                    "in": ["@faas.com", {"var": ["email"]}]
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 1);

        // Check that flag_set_metadata contains only the flattened "metadata" object
        // $schema and $evaluators should NOT be in flag_set_metadata
        assert!(!result.flag_set_metadata.contains_key("$schema"));
        assert!(!result.flag_set_metadata.contains_key("$evaluators"));

        // Metadata fields should be flattened
        assert_eq!(
            result.flag_set_metadata.get("environment"),
            Some(&json!("production"))
        );
        assert_eq!(result.flag_set_metadata.get("version"), Some(&json!(2)));
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
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("on".to_string()),
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
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("on".to_string()),
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
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("on".to_string()),
            variants: HashMap::new(),
            targeting: None,
            metadata: HashMap::new(),
        };

        let flag2 = FeatureFlag {
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("on".to_string()),
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
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("on".to_string()),
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
        assert_eq!(flag.default_variant.clone().unwrap(), "on");
        assert!(flag.targeting.is_some());
        assert_eq!(flag.metadata.get("key"), Some(&json!("value")));
    }

    #[test]
    fn test_flag_key_set_during_parsing() {
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                },
                "anotherFlag": {
                    "state": "DISABLED",
                    "variants": {"off": false},
                    "defaultVariant": "off"
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();

        // Verify keys are set correctly
        let test_flag = result.flags.get("testFlag").unwrap();
        assert_eq!(test_flag.key, Some("testFlag".to_string()));

        let another_flag = result.flags.get("anotherFlag").unwrap();
        assert_eq!(another_flag.key, Some("anotherFlag".to_string()));
    }

    #[test]
    fn test_evaluators_with_simple_ref() {
        let config = r#"{
            "$evaluators": {
                "isAdmin": {
                    "in": ["admin@", {"var": "email"}]
                }
            },
            "flags": {
                "adminFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"$ref": "isAdmin"},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 1);

        // Verify evaluators are used for $ref resolution (not stored in flag_set_metadata)
        assert!(!result.flag_set_metadata.contains_key("$evaluators"));

        // Verify the $ref was resolved in the targeting rule
        let flag = result.flags.get("adminFlag").unwrap();
        let targeting = flag.targeting.as_ref().unwrap();

        // The $ref should be replaced with the actual evaluator rule
        let targeting_obj = targeting.as_object().unwrap();
        assert!(targeting_obj.contains_key("if"));

        let if_array = targeting_obj.get("if").unwrap().as_array().unwrap();
        // First element should be the resolved evaluator (not a $ref)
        let first_elem = &if_array[0];
        assert!(first_elem.is_object());
        let first_obj = first_elem.as_object().unwrap();
        assert!(first_obj.contains_key("in"));
        assert!(!first_obj.contains_key("$ref"));
    }

    #[test]
    fn test_evaluators_with_nested_ref() {
        let config = r#"{
            "$evaluators": {
                "isAdmin": {
                    "in": ["admin@", {"var": "email"}]
                },
                "isEnabled": {
                    "and": [
                        {"$ref": "isAdmin"},
                        {"==": [{"var": "enabled"}, true]}
                    ]
                }
            },
            "flags": {
                "featureFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"$ref": "isEnabled"},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        let flag = result.flags.get("featureFlag").unwrap();
        let targeting = flag.targeting.as_ref().unwrap();

        // Verify nested $ref is resolved
        let targeting_str = targeting.to_string();
        assert!(!targeting_str.contains("$ref"));
        assert!(targeting_str.contains("in"));
        assert!(targeting_str.contains("admin@"));
    }

    #[test]
    fn test_evaluators_missing_ref_error() {
        let config = r#"{
            "$evaluators": {
                "isAdmin": {
                    "in": ["admin@", {"var": "email"}]
                }
            },
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"$ref": "nonExistentRule"},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("nonExistentRule"));
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_evaluators_circular_ref_error() {
        let config = r#"{
            "$evaluators": {
                "rule1": {
                    "$ref": "rule2"
                },
                "rule2": {
                    "$ref": "rule1"
                }
            },
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "off",
                    "targeting": {
                        "$ref": "rule1"
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Circular reference"));
    }

    #[test]
    fn test_evaluators_with_multiple_flags() {
        let config = r#"{
            "$evaluators": {
                "isAdmin": {
                    "in": ["admin@", {"var": "email"}]
                },
                "isPremium": {
                    "==": [{"var": "tier"}, "premium"]
                }
            },
            "flags": {
                "adminFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [{"$ref": "isAdmin"}, "on", "off"]
                    }
                },
                "premiumFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [{"$ref": "isPremium"}, "on", "off"]
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 2);

        // Both flags should have resolved $refs
        let admin_flag = result.flags.get("adminFlag").unwrap();
        let admin_targeting = admin_flag.targeting.as_ref().unwrap().to_string();
        assert!(!admin_targeting.contains("$ref"));
        assert!(admin_targeting.contains("in"));

        let premium_flag = result.flags.get("premiumFlag").unwrap();
        let premium_targeting = premium_flag.targeting.as_ref().unwrap().to_string();
        assert!(!premium_targeting.contains("$ref"));
        assert!(premium_targeting.contains("=="));
    }

    #[test]
    fn test_flags_without_evaluators() {
        // Flags should work fine without $evaluators
        let config = r#"{
            "flags": {
                "simpleFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "user"}, "admin"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let result = ParsingResult::parse(config).unwrap();
        assert_eq!(result.flags.len(), 1);
        let flag = result.flags.get("simpleFlag").unwrap();
        assert!(flag.targeting.is_some());
    }

    #[test]
    fn test_evaluators_with_complex_nested_structure() {
        let config = r#"{
            "$evaluators": {
                "baseRule": {
                    "==": [{"var": "status"}, "active"]
                },
                "compositeRule": {
                    "and": [
                        {"$ref": "baseRule"},
                        {">=": [{"var": "age"}, 18]}
                    ]
                }
            },
            "flags": {
                "complexFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {
                                "or": [
                                    {"$ref": "compositeRule"},
                                    {"==": [{"var": "override"}, true]}
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
        let flag = result.flags.get("complexFlag").unwrap();
        let targeting = flag.targeting.as_ref().unwrap();

        // Verify all $refs are resolved deeply
        let targeting_str = targeting.to_string();
        assert!(!targeting_str.contains("$ref"));
        assert!(targeting_str.contains("status"));
        assert!(targeting_str.contains("active"));
        assert!(targeting_str.contains("age"));
    }
}
