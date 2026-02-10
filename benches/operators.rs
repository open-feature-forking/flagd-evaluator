//! Benchmarks for custom JSON Logic operators.
//!
//! Measures the performance of fractional bucketing, semantic version comparison,
//! and string prefix/suffix matching operators through the DataLogic evaluation engine.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use flagd_evaluator::create_evaluator;

fn bench_fractional(c: &mut Criterion) {
    let logic = create_evaluator();
    let mut group = c.benchmark_group("fractional");

    // 2 buckets (typical A/B test)
    let rule_2 = r#"{"fractional": ["user-123", ["control", 50], ["treatment", 50]]}"#;
    let data = r#"{"targetingKey": "user-123", "$flagd": {"flagKey": "test"}}"#;
    group.bench_with_input(BenchmarkId::new("buckets", 2), &(), |b, _| {
        b.iter(|| logic.evaluate_json(black_box(rule_2), black_box(data)))
    });

    // 4 buckets
    let rule_4 = r#"{"fractional": ["user-123", ["a", 25], ["b", 25], ["c", 25], ["d", 25]]}"#;
    group.bench_with_input(BenchmarkId::new("buckets", 4), &(), |b, _| {
        b.iter(|| logic.evaluate_json(black_box(rule_4), black_box(data)))
    });

    // 8 buckets
    let rule_8 = r#"{"fractional": ["user-123", ["a", 12], ["b", 13], ["c", 12], ["d", 13], ["e", 12], ["f", 13], ["g", 12], ["h", 13]]}"#;
    group.bench_with_input(BenchmarkId::new("buckets", 8), &(), |b, _| {
        b.iter(|| logic.evaluate_json(black_box(rule_8), black_box(data)))
    });

    group.finish();
}

fn bench_semver_equals(c: &mut Criterion) {
    let logic = create_evaluator();
    let rule = r#"{"sem_ver": [{"var": "version"}, "=", "1.2.3"]}"#;
    let data = r#"{"version": "1.2.3"}"#;

    c.bench_function("semver_equals", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule), black_box(data)))
    });
}

fn bench_semver_range(c: &mut Criterion) {
    let logic = create_evaluator();
    let mut group = c.benchmark_group("semver_range");

    // Caret range
    let rule_caret = r#"{"sem_ver": [{"var": "version"}, "^", "1.2.0"]}"#;
    let data = r#"{"version": "1.5.3"}"#;
    group.bench_function("caret", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule_caret), black_box(data)))
    });

    // Tilde range
    let rule_tilde = r#"{"sem_ver": [{"var": "version"}, "~", "1.2.0"]}"#;
    let data_tilde = r#"{"version": "1.2.9"}"#;
    group.bench_function("tilde", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule_tilde), black_box(data_tilde)))
    });

    // Greater-than-or-equal with prerelease
    let rule_gte = r#"{"sem_ver": [{"var": "version"}, ">=", "2.0.0-alpha.1"]}"#;
    let data_gte = r#"{"version": "2.0.0-beta.1"}"#;
    group.bench_function("gte_prerelease", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule_gte), black_box(data_gte)))
    });

    group.finish();
}

fn bench_starts_with(c: &mut Criterion) {
    let logic = create_evaluator();
    let rule = r#"{"starts_with": [{"var": "email"}, "admin@"]}"#;
    let data = r#"{"email": "admin@example.com"}"#;

    c.bench_function("starts_with", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule), black_box(data)))
    });
}

fn bench_ends_with(c: &mut Criterion) {
    let logic = create_evaluator();
    let rule = r#"{"ends_with": [{"var": "email"}, "@example.com"]}"#;
    let data = r#"{"email": "user@example.com"}"#;

    c.bench_function("ends_with", |b| {
        b.iter(|| logic.evaluate_json(black_box(rule), black_box(data)))
    });
}

criterion_group!(
    benches,
    bench_fractional,
    bench_semver_equals,
    bench_semver_range,
    bench_starts_with,
    bench_ends_with,
);
criterion_main!(benches);
