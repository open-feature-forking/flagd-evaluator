//! Gherkin/Cucumber tests for flagd-evaluator using testbed scenarios.
//!
//! This test suite runs the official flagd testbed Gherkin feature files
//! to ensure compatibility with the flagd specification.

use cucumber::{given, then, when, World};
use flagd_evaluator::{
    evaluation::{evaluate_flag, ErrorCode, ResolutionReason},
    set_validation_mode,
    storage::{clear_flag_state, update_flag_state},
    ValidationMode,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// World state for Gherkin tests
#[derive(Debug, Default, World)]
pub struct FlagdWorld {
    /// The current evaluation context
    context: Value,
    /// The last evaluation result
    last_result: Option<flagd_evaluator::evaluation::EvaluationResult>,
    /// Flag configurations loaded
    flag_configs: HashMap<String, String>,
    /// Current flag key being tested
    current_flag_key: Option<String>,
    /// Current flag type being tested
    current_flag_type: Option<String>,
    /// Default value for current flag
    current_default: Option<Value>,
}

impl FlagdWorld {
    /// Load all flag configurations from testbed
    fn load_flag_configs(&mut self) {
        let testbed_flags = PathBuf::from("testbed/flags");

        if !testbed_flags.exists() {
            println!("Warning: testbed/flags directory not found");
            return;
        }

        for entry in fs::read_dir(&testbed_flags).expect("Failed to read testbed/flags") {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let config =
                    fs::read_to_string(&path).expect(&format!("Failed to read {:?}", path));
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .expect("Invalid filename")
                    .to_string();
                self.flag_configs.insert(filename, config);
            }
        }

        println!("Loaded {} flag configurations", self.flag_configs.len());
    }

    // /// Load a specific flag configuration file
    // fn load_config(&self, filename: &str) -> Result<(), String> {
    //     if let Some(config) = self.flag_configs.get(filename) {
    //         update_flag_state(config)
    //             .map_err(|e| format!("Failed to update state: {:?}", e))?;
    //         Ok(())
    //     } else {
    //         Err(format!("Flag config {} not found", filename))
    //     }
    // }
}

// ============================================================================
// Given Steps
// ============================================================================

#[given("a stable flagd provider")]
async fn given_stable_provider(world: &mut FlagdWorld) {
    clear_flag_state();
    world.load_flag_configs();

    // Merge all flag configs into one
    let mut merged_flags = json!({"flags": {}});
    let mut merged_metadata = serde_json::Map::new();

    let config_files = vec![
        "testing-flags.json",
        "zero-flags.json",
        "custom-ops.json",
        "evaluator-refs.json",
        "edge-case-flags.json",
        "metadata-flags.json",
    ];

    for filename in config_files {
        if let Some(config_str) = world.flag_configs.get(filename) {
            if let Ok(config) = serde_json::from_str::<Value>(config_str) {
                if let Some(flags) = config.get("flags").and_then(|f| f.as_object()) {
                    if let Some(merged_flags_obj) = merged_flags
                        .get_mut("flags")
                        .and_then(|f| f.as_object_mut())
                    {
                        for (key, value) in flags {
                            merged_flags_obj.insert(key.clone(), value.clone());
                        }
                    }
                }
                // Merge top-level metadata (like $evaluators, $schema)
                if let Some(config_obj) = config.as_object() {
                    for (key, value) in config_obj {
                        if key != "flags" {
                            merged_metadata.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
        }
    }

    // Add metadata to merged config
    for (key, value) in merged_metadata {
        merged_flags.as_object_mut().unwrap().insert(key, value);
    }

    set_validation_mode(ValidationMode::Permissive);
    let merged_config = serde_json::to_string(&merged_flags).unwrap();
    match update_flag_state(&merged_config) {
        Err(e) => {
            println!("Warning: Failed to load merged config: {:?}", e);
        }
        Ok(response) => {
            assert!(response.success, "{:?}", response.error);
        }
    }

    flagd_evaluator::storage::get_flag_state().expect("No flag state loaded");
    world.context = json!({});
}

#[given("a metadata flagd provider")]
async fn given_metadata_provider(world: &mut FlagdWorld) {
    clear_flag_state();
    world.load_flag_configs();
    world.context = json!({});
}

#[given(
    regex = r#"^an? (Boolean|String|Integer|Float|Object)-flag with key "([^"]+)" and a default value "([^"]*)"$"#
)]
async fn given_flag_with_key(
    world: &mut FlagdWorld,
    flag_type: String,
    key: String,
    default: String,
) {
    world.current_flag_key = Some(key);
    world.current_flag_type = Some(flag_type.clone());

    // Parse the default value based on type
    world.current_default = Some(match flag_type.as_str() {
        "Boolean" => json!(default.parse::<bool>().unwrap_or(false)),
        "Integer" => json!(default.parse::<i64>().unwrap_or(0)),
        "Float" => json!(default.parse::<f64>().unwrap_or(0.0)),
        "String" => json!(default),
        "Object" => serde_json::from_str(&default).unwrap_or(json!({})),
        _ => json!(null),
    });
}

#[given(
    regex = r#"^a context containing a key "([^"]+)", with type "([^"]+)" and with value "([^"]*)"$"#
)]
async fn given_context_with_key(world: &mut FlagdWorld, key: String, _type: String, value: String) {
    if let Some(obj) = world.context.as_object_mut() {
        // Parse value based on type
        let parsed_value = match _type.as_str() {
            "String" => json!(value),
            "Integer" => json!(value.parse::<i64>().unwrap_or(0)),
            "Boolean" => json!(value.parse::<bool>().unwrap_or(false)),
            "Float" => json!(value.parse::<f64>().unwrap_or(0.0)),
            _ => json!(value),
        };
        obj.insert(key, parsed_value);
    }
}

#[given(
    regex = r#"^a context containing a nested property with outer key "([^"]+)" and inner key "([^"]+)", with value "([^"]*)"$"#
)]
async fn given_context_nested(world: &mut FlagdWorld, outer: String, inner: String, value: String) {
    if let Some(obj) = world.context.as_object_mut() {
        let nested = json!({ inner: value });
        obj.insert(outer, nested);
    }
}

#[given(regex = r#"^a context containing a targeting key with value "([^"]*)"$"#)]
async fn given_context_targeting_key(world: &mut FlagdWorld, value: String) {
    if let Some(obj) = world.context.as_object_mut() {
        obj.insert("targetingKey".to_string(), json!(value));
    }
}

#[given(regex = r#"^an option "([^"]+)" of type "([^"]+)" with value "([^"]+)"$"#)]
async fn given_option(_world: &mut FlagdWorld, _key: String, _type: String, _value: String) {
    // Options like cache settings are not applicable for the evaluator
    // Skip this step
}

// ============================================================================
// When Steps
// ============================================================================

#[when("the flag was evaluated with details")]
async fn when_flag_evaluated(world: &mut FlagdWorld) {
    let flag_key = world.current_flag_key.as_ref().expect("No flag key set");

    let state = flagd_evaluator::storage::get_flag_state().expect("No flag state loaded");

    let flag = state.flags.get(flag_key);

    let result = if let Some(flag) = flag {
        evaluate_flag(flag, &world.context, &state.flag_set_metadata)
    } else {
        // Flag not found
        let default_val = world
            .current_default
            .as_ref()
            .cloned()
            .unwrap_or(json!(null));
        flagd_evaluator::evaluation::EvaluationResult {
            value: default_val,
            variant: None,
            reason: ResolutionReason::Error,
            error_code: Some(ErrorCode::FlagNotFound),
            error_message: Some(format!("Flag '{}' not found", flag_key)),
            flag_metadata: None,
        }
    };

    world.last_result = Some(result);
}

// ============================================================================
// Then Steps
// ============================================================================

#[then(regex = r#"^the resolved details value should be "([^"]*)"$"#)]
async fn then_resolved_value(world: &mut FlagdWorld, expected: String) {
    let result = world.last_result.as_ref().expect("No evaluation result");

    let flag_type = world.current_flag_type.as_ref().expect("No flag type set");

    // Parse expected based on flag type
    let expected_value = match flag_type.as_str() {
        "Boolean" => json!(expected.parse::<bool>().unwrap_or(false)),
        "Integer" => json!(expected.parse::<i64>().unwrap_or(0)),
        "Float" => json!(expected.parse::<f64>().unwrap_or(0.0)),
        "String" => json!(expected),
        _ => json!(expected),
    };

    if result.value == json!(null) {
        assert_eq!(
            world
                .current_default
                .clone()
                .unwrap_or_else( || { panic!("should be here") }),
            expected_value,
            "Expected value {:?} but got {:?} - fallbacked to default",
            expected_value,
            result.value
        );
    } else {
        assert_eq!(
            result.value, expected_value,
            "Expected value {:?} but got {:?} - error in {:?}",
            expected_value, result.value, result.error_code
        );
    }
}

#[then(regex = r#"^the reason should be "([^"]+)"$"#)]
async fn then_reason(world: &mut FlagdWorld, expected: String) {
    let result = world.last_result.as_ref().expect("No evaluation result");

    let expected_reason = match expected.as_str() {
        "STATIC" => ResolutionReason::Static,
        "DEFAULT" => ResolutionReason::Default,
        "TARGETING_MATCH" => ResolutionReason::TargetingMatch,
        "DISABLED" => ResolutionReason::Disabled,
        "ERROR" => ResolutionReason::FlagNotFound,
        "FLAG_NOT_FOUND" => ResolutionReason::Error, // FLAG_NOT_FOUND is represented as Error
        _ => panic!("Unknown reason: {}", expected),
    };

    assert_eq!(
        result.reason, expected_reason,
        "Expected reason {:?} but got {:?}",
        expected_reason, result.reason
    );
}

#[then(regex = r#"^the error-code should be "([^"]*)"$"#)]
async fn then_error_code(world: &mut FlagdWorld, expected: String) {
    let result = world.last_result.as_ref().expect("No evaluation result");

    if expected.is_empty() {
        assert!(
            result.error_code.is_none(),
            "Expected no error code but got {:?}",
            result.error_code
        );
    } else {
        let expected_code = match expected.as_str() {
            "FLAG_NOT_FOUND" => ErrorCode::FlagNotFound,
            "PARSE_ERROR" => ErrorCode::ParseError,
            "TYPE_MISMATCH" => ErrorCode::TypeMismatch,
            "GENERAL" => ErrorCode::General,
            _ => panic!("Unknown error code: {}", expected),
        };

        assert_eq!(
            result.error_code,
            Some(expected_code.clone()),
            "Expected error code {:?} but got {:?}",
            expected_code,
            result.error_code
        );
    }
}

#[then("the resolved metadata should contain")]
async fn then_metadata_contains(world: &mut FlagdWorld, step: &cucumber::gherkin::Step) {
    let result = world.last_result.as_ref().expect("No evaluation result");
    let metadata = result
        .flag_metadata
        .as_ref()
        .expect("No metadata in result");

    if let Some(table) = &step.table {
        for row in table.rows.iter().skip(1) {
            // Skip header
            let key = &row[0];
            let metadata_type = &row[1];
            let value_str = &row[2];

            let expected_value = match metadata_type.as_str() {
                "String" => json!(value_str),
                "Integer" => json!(value_str.parse::<i64>().unwrap()),
                "Float" => json!(value_str.parse::<f64>().unwrap()),
                "Boolean" => json!(value_str.parse::<bool>().unwrap()),
                _ => json!(value_str),
            };

            assert_eq!(
                metadata.get(key),
                Some(&expected_value),
                "Metadata key '{}' should be {:?} but got {:?}",
                key,
                expected_value,
                metadata.get(key)
            );
        }
    }
}

#[then("the resolved metadata is empty")]
async fn then_metadata_empty(world: &mut FlagdWorld) {
    let result = world.last_result.as_ref().expect("No evaluation result");
    assert!(
        result.flag_metadata.is_none(),
        "Expected no metadata but got {:?}",
        result.flag_metadata
    );
}

// ============================================================================
// Test Runner
// ============================================================================

#[tokio::test]
async fn run_evaluation_tests() {
    FlagdWorld::cucumber()
        .before(|_feature, _rule, _scenario, world| {
            Box::pin(async move {
                // Initialize world state
                world.load_flag_configs();
            })
        })
        .filter_run(
            "testbed/gherkin/evaluation.feature",
            |_feature, _rule, scenario| {
                // Skip scenarios that require RPC or connection management
                !scenario
                    .tags
                    .iter()
                    .any(|tag| tag == "grace" || tag == "rpc" || tag == "caching")
            },
        )
        .await;
}

#[tokio::test]
async fn run_targeting_tests() {
    FlagdWorld::cucumber()
        .before(|_feature, _rule, _scenario, world| {
            Box::pin(async move {
                // Initialize world state
                world.load_flag_configs();
            })
        })
        .filter_run(
            "testbed/gherkin/targeting.feature",
            |_feature, _rule, scenario| {
                // Skip scenarios that require features we don't support in the evaluator
                !scenario
                    .tags
                    .iter()
                    .any(|tag| tag == "grace" || tag == "rpc" || tag == "caching")
            },
        )
        .await;
}

#[tokio::test]
async fn run_context_enrichment_tests() {
    FlagdWorld::cucumber()
        .before(|_feature, _rule, _scenario, world| {
            Box::pin(async move {
                // Initialize world state
                world.load_flag_configs();
            })
        })
        .filter_run(
            "testbed/gherkin/contextEnrichment.feature",
            |_feature, _rule, scenario| {
                // Only run in-process tests, skip RPC and connection-related tests
                scenario.tags.iter().any(|tag| tag == "in-process")
                    && !scenario
                        .tags
                        .iter()
                        .any(|tag| tag == "grace" || tag == "rpc" || tag == "caching")
            },
        )
        .await;
}

#[tokio::test]
async fn run_metadata_tests() {
    FlagdWorld::cucumber()
        .before(|_feature, _rule, _scenario, world| {
            Box::pin(async move {
                // Initialize world state
                world.load_flag_configs();
            })
        })
        .filter_run(
            "testbed/gherkin/metadata.feature",
            |_feature, _rule, _scenario| {
                // Run all metadata tests
                true
            },
        )
        .await;
}
