//! Scale benchmarks for large flag stores (S6-S11).
//!
//! Tests update_state performance at 1K, 10K, and 100K flags, and verifies
//! O(1) evaluation lookup from a 10K-flag store for both static and targeting flags.
//! Flag distributions approximate production workloads: 70% static, 25% targeting, 5% disabled.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use flagd_evaluator::{FlagEvaluator, ValidationMode};
use serde_json::json;
use std::time::Duration;

/// Generates a flag configuration JSON string with the specified number of flags.
///
/// Distribution:
/// - 70% static (ENABLED, no targeting)
/// - 25% simple targeting (single `==` condition)
/// - 5% disabled
///
/// Flag names follow the pattern `flag_0001`, `flag_0002`, etc.
fn generate_scale_config(num_flags: usize) -> String {
    let mut buf = String::with_capacity(num_flags * 200);
    buf.push_str(r#"{"flags":{"#);

    for i in 0..num_flags {
        if i > 0 {
            buf.push(',');
        }

        let name = format!("flag_{:04}", i);
        let category = i % 20; // deterministic distribution

        if category < 14 {
            // 70% static flags (0-13 out of 0-19)
            buf.push_str(&format!(
                r#""{name}":{{"state":"ENABLED","variants":{{"on":true,"off":false}},"defaultVariant":"on"}}"#
            ));
        } else if category < 19 {
            // 25% targeting flags (14-18 out of 0-19)
            buf.push_str(&format!(
                r#""{name}":{{"state":"ENABLED","variants":{{"on":true,"off":false}},"defaultVariant":"off","targeting":{{"if":[{{"==":[{{"var":"color"}},"blue"]}},"on","off"]}}}}"#
            ));
        } else {
            // 5% disabled flags (19 out of 0-19)
            buf.push_str(&format!(
                r#""{name}":{{"state":"DISABLED","variants":{{"on":true,"off":false}},"defaultVariant":"on"}}"#
            ));
        }
    }

    buf.push_str("}}");
    buf
}

// ---------------------------------------------------------------------------
// S6-S8: update_state at scale
// ---------------------------------------------------------------------------

fn bench_update_state_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_update_state");

    for &(size, label, sample_size) in &[
        (1_000, "S6_1K", 50),
        (10_000, "S7_10K", 10),
        (100_000, "S8_100K", 10),
    ] {
        let config = generate_scale_config(size);

        group.sample_size(sample_size);
        // Give S8 more time to complete
        if size >= 100_000 {
            group.measurement_time(Duration::from_secs(30));
        } else if size >= 10_000 {
            group.measurement_time(Duration::from_secs(15));
        }

        group.bench_with_input(BenchmarkId::new("fresh", label), &config, |b, config| {
            b.iter_batched(
                || FlagEvaluator::new(ValidationMode::Permissive),
                |mut evaluator| {
                    evaluator.update_state(black_box(config)).unwrap();
                },
                criterion::BatchSize::PerIteration,
            )
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// S10-S11: Evaluate from a large (10K) flag store
// ---------------------------------------------------------------------------

/// S10: Evaluate a static flag from a 10K-flag store.
/// Verifies that flag lookup is O(1) regardless of store size.
fn bench_evaluate_static_from_10k(c: &mut Criterion) {
    let config = generate_scale_config(10_000);
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(&config).unwrap();

    // flag_0000 is a static flag (index 0, 0 % 20 == 0, which is < 14 -> static)
    let context = json!({});

    c.bench_function("S10_evaluate_static_from_10K_store", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("flag_0000"), black_box(&context)))
    });
}

/// S11: Evaluate a targeting flag from a 10K-flag store.
/// Verifies that targeting evaluation is O(1) regardless of store size.
fn bench_evaluate_targeting_from_10k(c: &mut Criterion) {
    let config = generate_scale_config(10_000);
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(&config).unwrap();

    // flag_0014 is a targeting flag (index 14, 14 % 20 == 14, which is >= 14 and < 19 -> targeting)
    let context = json!({"color": "blue", "targetingKey": "user-123"});

    c.bench_function("S11_evaluate_targeting_from_10K_store", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("flag_0014"), black_box(&context)))
    });
}

criterion_group!(
    benches,
    bench_update_state_scale,
    bench_evaluate_static_from_10k,
    bench_evaluate_targeting_from_10k,
);
criterion_main!(benches);
