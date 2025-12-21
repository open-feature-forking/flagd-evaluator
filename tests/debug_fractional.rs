//! Debug test for fractional operator in Gherkin scenarios

use flagd_evaluator::*;
use serde_json::json;

#[test]
fn debug_fractional_operator() {
    storage::clear_flag_state();

    let config = r#"{
        "flags": {
            "fractional-flag": {
                "state": "ENABLED",
                "variants": {
                    "clubs": "clubs",
                    "diamonds": "diamonds",
                    "hearts": "hearts",
                    "spades": "spades",
                    "wild": "wild"
                },
                "defaultVariant": "wild",
                "targeting": {
                    "fractional": [
                        {"cat": [
                            { "var": "$flagd.flagKey" },
                            { "var": "user.name" }
                        ]},
                        [ "clubs", 25 ],
                        [ "diamonds", 25 ],
                        [ "hearts", 25 ],
                        [ "spades", 25 ]
                    ]
                }
            }
        }
    }"#;

    let update_result = storage::update_flag_state(config);
    println!("Update result: {:?}", update_result);
    assert!(update_result.is_ok());

    let state = storage::get_flag_state().unwrap();
    let flag = state.flags.get("fractional-flag").unwrap();

    println!("Flag: {:?}", flag);
    println!("Targeting: {:?}", flag.targeting);

    let context = json!({"user": {"name": "jack"}});
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

    // The test expects "spades" for user.name = "jack"
    assert_eq!(result.value, json!("spades"), "Expected 'spades' for jack");
}

#[test]
fn debug_fractional_direct() {
    // Test fractional operator directly
    let bucket_key = "fractional-flagjack";
    let buckets = vec![
        json!(["clubs", 25]),
        json!(["diamonds", 25]),
        json!(["hearts", 25]),
        json!(["spades", 25]),
    ];

    let result = operators::fractional(bucket_key, &buckets);
    println!("Direct fractional result: {:?}", result);
    assert!(result.is_ok());
    println!("Bucket for 'jack': {}", result.unwrap());
}
