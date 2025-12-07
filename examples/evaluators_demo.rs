//! Demo of $evaluators and $ref resolution
//!
//! Run with: cargo run --example evaluators_demo

use flagd_evaluator::evaluation::evaluate_flag;
use flagd_evaluator::storage::{get_flag_state, update_flag_state};
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
    update_flag_state(config).expect("Failed to update state");

    let state = get_flag_state().expect("Failed to get state");
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
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    println!("1️⃣  Admin user (admin@company.com, tier=basic):");
    println!(
        "   → Result: {}, Variant: {}",
        result.value,
        result.variant.unwrap()
    );

    // Test 2: Premium user
    let context = json!({"email": "user@company.com", "tier": "premium"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    println!("\n2️⃣  Premium user (user@company.com, tier=premium):");
    println!(
        "   → Result: {}, Variant: {}",
        result.value,
        result.variant.unwrap()
    );

    // Test 3: Regular user
    let context = json!({"email": "user@company.com", "tier": "basic"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    println!("\n3️⃣  Regular user (user@company.com, tier=basic):");
    println!(
        "   → Result: {}, Variant: {}",
        result.value,
        result.variant.unwrap()
    );

    println!("\n✅ All evaluations completed successfully!");
}
