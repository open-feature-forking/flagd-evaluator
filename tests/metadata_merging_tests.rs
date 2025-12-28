//! Tests for metadata merging in flag evaluation responses.
//!
//! According to the flagd provider specification, evaluation responses should merge
//! flag-set metadata and flag-level metadata, with flag metadata taking priority.
//! Metadata should be returned on a "best effort" basis for disabled, missing, and
//! erroneous flags.

use flagd_evaluator::{FlagEvaluator, ValidationMode};
use serde_json::json;

#[test]
fn test_metadata_merging_flag_priority() {
    // Test that flag metadata takes priority over flag-set metadata
    let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

    let config = r#"{
        "metadata": {
            "version": "1.0",
            "env": "production",
            "owner": "flagset"
        },
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true,
                    "off": false
                },
                "metadata": {
                    "owner": "flag-owner",
                    "description": "Test flag"
                }
            }
        }
    }"#;

    evaluator.update_state(config).unwrap();

    let context = json!({});
    let result = evaluator.evaluate_flag("testFlag", &context);

    // Verify metadata is merged with flag metadata taking priority
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();

    // Flag metadata should override flag-set metadata
    assert_eq!(metadata.get("owner").unwrap(), "flag-owner");

    // Flag metadata should be included
    assert_eq!(metadata.get("description").unwrap(), "Test flag");

    // Flag-set metadata should be included where not overridden
    assert_eq!(metadata.get("version").unwrap(), "1.0");
    assert_eq!(metadata.get("env").unwrap(), "production");
}

#[test]
fn test_metadata_only_flag_set() {
    // Test when only flag-set metadata exists
    let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

    let config = r#"{
        "metadata": {
            "version": "2.0",
            "team": "platform"
        },
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true,
                    "off": false
                }
            }
        }
    }"#;

    evaluator.update_state(config).unwrap();

    let context = json!({});
    let result = evaluator.evaluate_flag("testFlag", &context);

    // Verify flag-set metadata is included
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("version").unwrap(), "2.0");
    assert_eq!(metadata.get("team").unwrap(), "platform");
}

#[test]
fn test_metadata_only_flag_level() {
    // Test when only flag-level metadata exists
    let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

    let config = r#"{
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true,
                    "off": false
                },
                "metadata": {
                    "deprecated": false,
                    "contact": "team@example.com"
                }
            }
        }
    }"#;

    evaluator.update_state(config).unwrap();

    let context = json!({});
    let result = evaluator.evaluate_flag("testFlag", &context);

    // Verify flag-level metadata is included
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("deprecated").unwrap(), false);
    assert_eq!(metadata.get("contact").unwrap(), "team@example.com");
}

#[test]
fn test_metadata_disabled_flag_returns_metadata() {
    // Test that metadata is returned even for disabled flags
    let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

    let config = r#"{
        "metadata": {
            "version": "1.0"
        },
        "flags": {
            "testFlag": {
                "state": "DISABLED",
                "defaultVariant": "off",
                "variants": {
                    "on": true,
                    "off": false
                },
                "metadata": {
                    "reason": "Feature discontinued"
                }
            }
        }
    }"#;

    evaluator.update_state(config).unwrap();

    let context = json!({});
    let result = evaluator.evaluate_flag("testFlag", &context);

    // Verify metadata is returned even for disabled flag
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("version").unwrap(), "1.0");
    assert_eq!(metadata.get("reason").unwrap(), "Feature discontinued");
}

#[test]
fn test_metadata_missing_flag_returns_flag_set_metadata() {
    // Test that flag-set metadata is returned even when flag doesn't exist
    let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

    let config = r#"{
        "metadata": {
            "version": "1.0",
            "fallback": "Use default"
        },
        "flags": {
            "existingFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true
                }
            }
        }
    }"#;

    evaluator.update_state(config).unwrap();

    let context = json!({});
    let result = evaluator.evaluate_flag("missingFlag", &context);

    // Verify flag-set metadata is returned on "best effort" basis
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("version").unwrap(), "1.0");
    assert_eq!(metadata.get("fallback").unwrap(), "Use default");
}

#[test]
fn test_metadata_empty_merging() {
    // Test when both flag-set and flag metadata are empty
    let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

    let config = r#"{
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true,
                    "off": false
                }
            }
        }
    }"#;

    evaluator.update_state(config).unwrap();

    let context = json!({});
    let result = evaluator.evaluate_flag("testFlag", &context);

    // Verify empty metadata is not included
    assert!(result.flag_metadata.is_none() || result.flag_metadata.as_ref().unwrap().is_empty());
}
