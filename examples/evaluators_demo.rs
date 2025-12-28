//! Demo of $evaluators and $ref resolution
//!
//! Run with: cargo run --example evaluators_demo

use flagd_evaluator::{FlagEvaluator, ValidationMode};
use serde_json::json;

fn main() {
    let config = r#"{
        "$evaluators": {
            "isAdmin": {
                "starts_with": [{"var": "email"}, "admin@"]
            },
            "isPremium": {
                "==": [{"var": "tier"}, "premium"]
            },
            "isVIP": {
                "or": [
                    {"$ref": "isAdmin"},
                    {"$ref": "isPremium"}
                ]
            }
        },
        "flags": {
            "vipFeatures": {
                "state": "ENABLED",
                "variants": {
                    "enabled": true,
                    "disabled": false
                },
                "defaultVariant": "disabled",
                "targeting": {
                    "if": [
                        {"$ref": "isVIP"},
                        "enabled",
                        "disabled"
                    ]
                }
            }
        }
    }"#;

    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  Evaluators Demo: $evaluators and $ref Resolution        ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    println!("Loading flag configuration with $evaluators...");

    // Create an evaluator instance
    let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);
    evaluator
        .update_state(config)
        .expect("Failed to update state");

    let state = evaluator.get_state().expect("Failed to get state");
    let flag = state.flags.get("vipFeatures").expect("Flag not found");

    println!("✓ Configuration loaded successfully");
    println!("✓ Flag 'vipFeatures' found");
    println!("\n📝 Resolved targeting rule (with $refs replaced):");
    println!(
        "{}\n",
        serde_json::to_string_pretty(&flag.targeting).unwrap()
    );

    println!("🧪 Testing evaluation scenarios:\n");

    // Test 1: Admin user
    let context = json!({"email": "admin@company.com", "tier": "basic"});
    let result = evaluator.evaluate_flag("vipFeatures", &context);
    println!("1️⃣  Admin user (admin@company.com, tier=basic):");
    println!(
        "   → Result: {}, Variant: {}",
        result.value,
        result.variant.unwrap()
    );

    // Test 2: Premium user
    let context = json!({"email": "user@company.com", "tier": "premium"});
    let result = evaluator.evaluate_flag("vipFeatures", &context);
    println!("\n2️⃣  Premium user (user@company.com, tier=premium):");
    println!(
        "   → Result: {}, Variant: {}",
        result.value,
        result.variant.unwrap()
    );

    // Test 3: Regular user
    let context = json!({"email": "user@company.com", "tier": "basic"});
    let result = evaluator.evaluate_flag("vipFeatures", &context);
    println!("\n3️⃣  Regular user (user@company.com, tier=basic):");
    println!(
        "   → Result: {}, Variant: {}",
        result.value,
        result.variant.unwrap()
    );

    println!("\n✅ All evaluations completed successfully!");
}
