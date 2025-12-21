//! Debug test for targeting key evaluation

use flagd_evaluator::*;
use serde_json::json;

#[test]
fn debug_targeting_key_matching() {
    storage::clear_flag_state();

    let config = r#"{
        "flags": {
            "targeting-key-flag": {
                "state": "ENABLED",
                "variants": {
                    "miss": "miss",
                    "hit": "hit"
                },
                "defaultVariant": "miss",
                "targeting": {
                    "if": [
                        {
                            "==": [{"var": "targetingKey"}, "5c3d8535-f81a-4478-a6d3-afaa4d51199e"]
                        },
                        "hit",
                        null
                    ]
                }
            }
        }
    }"#;

    storage::update_flag_state(config).expect("Failed to load config");
    let state = storage::get_flag_state().unwrap();
    let flag = state.flags.get("targeting-key-flag").unwrap();

    // Test with correct targeting key
    let context = json!({"targetingKey": "5c3d8535-f81a-4478-a6d3-afaa4d51199e"});
    println!("Context: {}", serde_json::to_string_pretty(&context).unwrap());

    let result = evaluation::evaluate_flag(flag, &context, &state.flag_set_metadata);

    println!("\n=== EVALUATION RESULT ===");
    println!("Value: {}", result.value);
    println!("Variant: {:?}", result.variant);
    println!("Reason: {:?}", result.reason);
    println!("Error code: {:?}", result.error_code);
    if let Some(err) = &result.error_message {
        println!("Error message: {}", err);
    }

    assert_eq!(result.value, json!("hit"), "Expected 'hit' for correct targeting key");
    assert_eq!(result.reason, evaluation::ResolutionReason::TargetingMatch);

    // Test with incorrect targeting key
    let context2 = json!({"targetingKey": "f20bd32d-703b-48b6-bc8e-79d53c85134a"});
    let result2 = evaluation::evaluate_flag(flag, &context2, &state.flag_set_metadata);

    println!("\n=== SECOND TEST ===");
    println!("Value: {}", result2.value);
    println!("Variant: {:?}", result2.variant);
    println!("Reason: {:?}", result2.reason);

    assert_eq!(result2.value, json!("miss"), "Expected 'miss' for incorrect targeting key");
    assert_eq!(result2.reason, evaluation::ResolutionReason::Default);
}
