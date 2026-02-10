//! Benchmarks for flag state management.
//!
//! Measures the performance of update_state with varying configuration sizes,
//! including change detection overhead when re-applying identical or slightly modified configs.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use flagd_evaluator::{FlagEvaluator, ValidationMode};

/// Generates a flag configuration JSON string with the specified number of flags.
fn generate_config(num_flags: usize) -> String {
    let mut flags = Vec::with_capacity(num_flags);
    for i in 0..num_flags {
        flags.push(format!(
            r#""flag_{i}": {{
                "state": "ENABLED",
                "variants": {{
                    "on": true,
                    "off": false
                }},
                "defaultVariant": "on",
                "targeting": {{
                    "if": [
                        {{"==": [{{"var": "user_id"}}, "user-{i}"]}},
                        "on",
                        "off"
                    ]
                }}
            }}"#,
            i = i
        ));
    }
    format!(r#"{{"flags": {{{}}}}}"#, flags.join(","))
}

/// Generates a config identical to generate_config but with one flag's default changed.
fn generate_config_one_change(num_flags: usize) -> String {
    let mut flags = Vec::with_capacity(num_flags);
    for i in 0..num_flags {
        let default = if i == 0 { "off" } else { "on" };
        flags.push(format!(
            r#""flag_{i}": {{
                "state": "ENABLED",
                "variants": {{
                    "on": true,
                    "off": false
                }},
                "defaultVariant": "{default}",
                "targeting": {{
                    "if": [
                        {{"==": [{{"var": "user_id"}}, "user-{i}"]}},
                        "on",
                        "off"
                    ]
                }}
            }}"#,
            i = i,
            default = default
        ));
    }
    format!(r#"{{"flags": {{{}}}}}"#, flags.join(","))
}

fn bench_update_state_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_state");

    for &size in &[5, 50, 200] {
        let config = generate_config(size);
        let label = match size {
            5 => "small_5",
            50 => "medium_50",
            200 => "large_200",
            _ => unreachable!(),
        };

        group.bench_with_input(BenchmarkId::new("fresh", label), &config, |b, config| {
            b.iter_batched(
                || FlagEvaluator::new(ValidationMode::Permissive),
                |mut evaluator| {
                    evaluator.update_state(black_box(config)).unwrap();
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_update_state_no_change(c: &mut Criterion) {
    let config = generate_config(100);

    c.bench_function("update_state_no_change", |b| {
        b.iter_batched(
            || {
                let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
                evaluator.update_state(&config).unwrap();
                evaluator
            },
            |mut evaluator| {
                evaluator.update_state(black_box(&config)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_update_state_incremental(c: &mut Criterion) {
    let config_base = generate_config(100);
    let config_changed = generate_config_one_change(100);

    c.bench_function("update_state_incremental", |b| {
        b.iter_batched(
            || {
                let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
                evaluator.update_state(&config_base).unwrap();
                evaluator
            },
            |mut evaluator| {
                evaluator.update_state(black_box(&config_changed)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_update_state_sizes,
    bench_update_state_no_change,
    bench_update_state_incremental,
);
criterion_main!(benches);
