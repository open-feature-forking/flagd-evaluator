//! Benchmarks for core flag evaluation logic.
//!
//! Measures the performance of evaluating flags through the FlagEvaluator API,
//! covering static resolution, targeting matches, disabled flags, and error paths.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flagd_evaluator::{create_evaluator, FlagEvaluator, ValidationMode};
use serde_json::json;

/// Flag configuration containing multiple flag types for benchmarking.
const BENCH_CONFIG: &str = r#"{
    "flags": {
        "boolFlag": {
            "state": "ENABLED",
            "variants": {
                "on": true,
                "off": false
            },
            "defaultVariant": "on"
        },
        "targetedFlag": {
            "state": "ENABLED",
            "variants": {
                "admin": "admin-value",
                "user": "user-value"
            },
            "defaultVariant": "user",
            "targeting": {
                "if": [
                    {"==": [{"var": "role"}, "admin"]},
                    "admin",
                    "user"
                ]
            }
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
        },
        "disabledFlag": {
            "state": "DISABLED",
            "variants": {
                "on": true,
                "off": false
            },
            "defaultVariant": "on"
        }
    }
}"#;

fn evaluate_flag_simple(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({});

    c.bench_function("evaluate_flag_simple", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("boolFlag"), black_box(&context)))
    });
}

fn evaluate_flag_targeting_match(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({"role": "admin"});

    c.bench_function("evaluate_flag_targeting_match", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("targetedFlag"), black_box(&context)))
    });
}

fn evaluate_flag_targeting_no_match(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({"role": "viewer"});

    c.bench_function("evaluate_flag_targeting_no_match", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("targetedFlag"), black_box(&context)))
    });
}

fn evaluate_flag_complex_targeting(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({"tier": "standard", "score": 75});

    c.bench_function("evaluate_flag_complex_targeting", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("complexFlag"), black_box(&context)))
    });
}

fn evaluate_flag_disabled(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({});

    c.bench_function("evaluate_flag_disabled", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("disabledFlag"), black_box(&context)))
    });
}

fn evaluate_flag_not_found(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({});

    c.bench_function("evaluate_flag_not_found", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("nonexistent"), black_box(&context)))
    });
}

fn evaluate_logic_simple(c: &mut Criterion) {
    let logic = create_evaluator();
    let rule = r#"{"==":[1,1]}"#;
    let data = r#"{}"#;

    c.bench_function("evaluate_logic_simple", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule), black_box(data)))
    });
}

fn evaluate_logic_complex(c: &mut Criterion) {
    let logic = create_evaluator();
    let rule = r#"{
        "if": [
            {"and": [
                {">":[{"var":"age"}, 18]},
                {"==":[{"var":"country"}, "US"]},
                {"starts_with":[{"var":"email"}, "admin"]}
            ]},
            "eligible",
            {"if": [
                {"or": [
                    {"sem_ver": [{"var":"appVersion"}, ">=", "2.0.0"]},
                    {"ends_with": [{"var":"email"}, "@beta.com"]}
                ]},
                "beta",
                "ineligible"
            ]}
        ]
    }"#;
    let data = r#"{"age":25,"country":"US","email":"admin@example.com","appVersion":"2.1.0"}"#;

    c.bench_function("evaluate_logic_complex", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule), black_box(data)))
    });
}

criterion_group!(
    benches,
    evaluate_flag_simple,
    evaluate_flag_targeting_match,
    evaluate_flag_targeting_no_match,
    evaluate_flag_complex_targeting,
    evaluate_flag_disabled,
    evaluate_flag_not_found,
    evaluate_logic_simple,
    evaluate_logic_complex,
);
criterion_main!(benches);
