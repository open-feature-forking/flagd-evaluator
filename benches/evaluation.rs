//! Benchmarks for core flag evaluation logic.
//!
//! Measures the performance of evaluating flags through the FlagEvaluator API,
//! covering static resolution, targeting matches, disabled flags, error paths,
//! and the impact of context size on evaluation performance.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flagd_evaluator::{create_evaluator, FlagEvaluator, ValidationMode};
use serde_json::{json, Map, Value};

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

// ---------------------------------------------------------------------------
// Context size variations (E2-E7 from BENCHMARKS.md)
// ---------------------------------------------------------------------------

/// Standard small context (5 attributes) from BENCHMARKS.md.
fn small_context() -> Value {
    json!({
        "targetingKey": "user-123",
        "tier": "premium",
        "role": "admin",
        "region": "us-east",
        "score": 85
    })
}

/// Standard large context (100+ attributes) from BENCHMARKS.md.
/// Includes the small context attributes plus `attr_0` through `attr_99`.
fn large_context() -> Value {
    let mut map = Map::new();
    map.insert("targetingKey".into(), Value::String("user-123".into()));
    map.insert("tier".into(), Value::String("premium".into()));
    map.insert("role".into(), Value::String("admin".into()));
    map.insert("region".into(), Value::String("us-east".into()));
    map.insert("score".into(), Value::Number(85.into()));
    for i in 0..100 {
        map.insert(format!("attr_{}", i), Value::String(format!("value_{}", i)));
    }
    Value::Object(map)
}

/// E2: Simple flag with small context - measures serialization overhead for a typical call.
fn evaluate_flag_simple_small_ctx(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = small_context();

    c.bench_function("evaluate_flag_simple_small_ctx", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("boolFlag"), black_box(&context)))
    });
}

/// E3: Simple flag with large context - measures serialization cost dominance.
fn evaluate_flag_simple_large_ctx(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = large_context();

    c.bench_function("evaluate_flag_simple_large_ctx", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("boolFlag"), black_box(&context)))
    });
}

/// E4: Simple targeting with small context - measures minimal rule evaluation cost.
fn evaluate_flag_targeting_small_ctx(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = small_context(); // contains "role": "admin" which triggers the match

    c.bench_function("evaluate_flag_targeting_small_ctx", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("targetedFlag"), black_box(&context)))
    });
}

/// E5: Simple targeting with large context - measures targeting evaluation with large payloads.
fn evaluate_flag_targeting_large_ctx(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = large_context(); // contains "role": "admin" which triggers the match

    c.bench_function("evaluate_flag_targeting_large_ctx", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("targetedFlag"), black_box(&context)))
    });
}

/// E6: Complex targeting with small context - measures rule evaluation cost dominance.
fn evaluate_flag_complex_targeting_small_ctx(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = small_context(); // contains "tier": "premium" and "score": 85

    c.bench_function("evaluate_flag_complex_targeting_small_ctx", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("complexFlag"), black_box(&context)))
    });
}

/// E7: Complex targeting with large context - worst case scenario.
fn evaluate_flag_complex_targeting_large_ctx(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = large_context(); // contains "tier": "premium" and "score": 85

    c.bench_function("evaluate_flag_complex_targeting_large_ctx", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("complexFlag"), black_box(&context)))
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
    // Context size variations
    evaluate_flag_simple_small_ctx,
    evaluate_flag_simple_large_ctx,
    evaluate_flag_targeting_small_ctx,
    evaluate_flag_targeting_large_ctx,
    evaluate_flag_complex_targeting_small_ctx,
    evaluate_flag_complex_targeting_large_ctx,
);
criterion_main!(benches);
