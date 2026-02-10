//! Comparison benchmarks: DataLogic direct evaluation vs FlagEvaluator.
//!
//! Measures the overhead that FlagEvaluator adds on top of raw DataLogic
//! evaluation (state lookup, context enrichment with $flagd properties, etc.).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flagd_evaluator::{create_evaluator, FlagEvaluator, ValidationMode};
use serde_json::json;

/// Flag configuration for FlagEvaluator benchmarks.
const BENCH_CONFIG: &str = r#"{
    "flags": {
        "simpleFlag": {
            "state": "ENABLED",
            "variants": {
                "on": true,
                "off": false
            },
            "defaultVariant": "on"
        },
        "complexFlag": {
            "state": "ENABLED",
            "variants": {
                "premium": "premium-tier",
                "standard": "standard-tier",
                "basic": "basic-tier"
            },
            "defaultVariant": "basic",
            "targeting": {
                "if": [
                    {"and": [
                        {"==": [{"var": "tier"}, "premium"]},
                        {">": [{"var": "score"}, 90]}
                    ]},
                    "premium",
                    {"if": [
                        {"or": [
                            {"==": [{"var": "tier"}, "standard"]},
                            {">": [{"var": "score"}, 50]}
                        ]},
                        "standard",
                        "basic"
                    ]}
                ]
            }
        }
    }
}"#;

// ---------------------------------------------------------------------------
// X1: Simple rule - DataLogic vs FlagEvaluator
// ---------------------------------------------------------------------------

/// X1a: Direct DataLogic evaluation of a trivial rule.
/// Measures the raw JSON Logic engine performance.
fn comparison_simple_datalogic(c: &mut Criterion) {
    let logic = create_evaluator();
    let rule = r#"{"==":[1,1]}"#;
    let data = r#"{}"#;

    c.bench_function("comparison_simple_datalogic", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule), black_box(data)))
    });
}

/// X1b: FlagEvaluator evaluation of a simple (non-targeting) flag.
/// Measures DataLogic + state lookup + context enrichment overhead.
fn comparison_simple_flag_evaluator(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({});

    c.bench_function("comparison_simple_flag_evaluator", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("simpleFlag"), black_box(&context)))
    });
}

// ---------------------------------------------------------------------------
// X2: Complex targeting rule - DataLogic vs FlagEvaluator
// ---------------------------------------------------------------------------

/// X2a: Direct DataLogic evaluation of the same complex targeting rule
/// used in the FlagEvaluator benchmark. The data includes $flagd properties
/// that would be injected by the evaluator, so the rule logic is equivalent.
fn comparison_complex_datalogic(c: &mut Criterion) {
    let logic = create_evaluator();
    let rule = r#"{
        "if": [
            {"and": [
                {"==": [{"var": "tier"}, "premium"]},
                {">": [{"var": "score"}, 90]}
            ]},
            "premium",
            {"if": [
                {"or": [
                    {"==": [{"var": "tier"}, "standard"]},
                    {">": [{"var": "score"}, 50]}
                ]},
                "standard",
                "basic"
            ]}
        ]
    }"#;
    let data = r#"{"tier":"standard","score":75,"targetingKey":"","$flagd":{"flagKey":"complexFlag","timestamp":1700000000}}"#;

    c.bench_function("comparison_complex_datalogic", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule), black_box(data)))
    });
}

/// X2b: FlagEvaluator evaluation of a complex flag with nested targeting rules.
/// Includes state lookup, context enrichment, and variant resolution overhead.
fn comparison_complex_flag_evaluator(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({"tier": "standard", "score": 75});

    c.bench_function("comparison_complex_flag_evaluator", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("complexFlag"), black_box(&context)))
    });
}

criterion_group!(
    benches,
    comparison_simple_datalogic,
    comparison_simple_flag_evaluator,
    comparison_complex_datalogic,
    comparison_complex_flag_evaluator,
);
criterion_main!(benches);
