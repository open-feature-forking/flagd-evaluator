//! Benchmarks for concurrent flag evaluation.
//!
//! Measures the performance of evaluating flags under multi-threaded contention,
//! covering simple and targeting evaluations, mixed workloads, and read/write
//! contention from concurrent update_state calls.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flagd_evaluator::{FlagEvaluator, ValidationMode};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::thread;

/// Flag configuration used by concurrency benchmarks.
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

/// Alternative config used for the read/write contention benchmark.
/// Identical structure but with a different default variant for boolFlag.
const BENCH_CONFIG_ALT: &str = r#"{
    "flags": {
        "boolFlag": {
            "state": "ENABLED",
            "variants": {
                "on": true,
                "off": false
            },
            "defaultVariant": "off"
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

fn make_evaluator() -> Arc<Mutex<FlagEvaluator>> {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    Arc::new(Mutex::new(evaluator))
}

// ---------------------------------------------------------------------------
// C1: Single-threaded baseline
// ---------------------------------------------------------------------------

/// C1: Single-threaded evaluation baseline for comparison with concurrent benchmarks.
fn concurrent_simple_1t(c: &mut Criterion) {
    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    evaluator.update_state(BENCH_CONFIG).unwrap();
    let context = json!({});

    c.bench_function("concurrent_simple_1t", |b| {
        b.iter(|| evaluator.evaluate_flag(black_box("boolFlag"), black_box(&context)))
    });
}

// ---------------------------------------------------------------------------
// C2: 4 threads evaluating simple flag
// ---------------------------------------------------------------------------

/// C2: 4 threads concurrently evaluating a simple (static) flag.
fn concurrent_simple_4t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    c.bench_function("concurrent_simple_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let eval = Arc::clone(&evaluator);
                    thread::spawn(move || {
                        let ctx = json!({});
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box("boolFlag"), black_box(&ctx))
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C3: 8 threads evaluating simple flag
// ---------------------------------------------------------------------------

/// C3: 8 threads concurrently evaluating a simple (static) flag.
fn concurrent_simple_8t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    c.bench_function("concurrent_simple_8t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let eval = Arc::clone(&evaluator);
                    thread::spawn(move || {
                        let ctx = json!({});
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box("boolFlag"), black_box(&ctx))
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C4: 4 threads evaluating targeting flag
// ---------------------------------------------------------------------------

/// C4: 4 threads concurrently evaluating a flag with targeting rules.
fn concurrent_targeting_4t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    c.bench_function("concurrent_targeting_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|i| {
                    let eval = Arc::clone(&evaluator);
                    thread::spawn(move || {
                        let role = if i % 2 == 0 { "admin" } else { "viewer" };
                        let ctx = json!({"role": role});
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box("targetedFlag"), black_box(&ctx))
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C5: Mixed workload - 4 threads randomly pick simple/targeting/disabled
// ---------------------------------------------------------------------------

/// C5: 4 threads with mixed workload (simple, targeting, and disabled flags).
fn concurrent_mixed_4t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    // Pre-define the flag/context pairs each thread will use.
    // Thread 0: simple, Thread 1: targeting, Thread 2: disabled, Thread 3: targeting
    let workloads: Vec<(&str, Value)> = vec![
        ("boolFlag", json!({})),
        ("targetedFlag", json!({"role": "admin"})),
        ("disabledFlag", json!({})),
        ("targetedFlag", json!({"role": "viewer"})),
    ];

    c.bench_function("concurrent_mixed_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = workloads
                .iter()
                .map(|(flag_key, ctx)| {
                    let eval = Arc::clone(&evaluator);
                    let key = *flag_key;
                    let context = ctx.clone();
                    thread::spawn(move || {
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box(key), black_box(&context))
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C6: Read/write contention - 3 readers + 1 writer
// ---------------------------------------------------------------------------

/// C6: Read/write contention - 3 threads evaluating while 1 thread updates state.
///
/// The writer thread alternates between two configurations on each iteration,
/// simulating periodic config refreshes typical in production environments.
fn concurrent_read_write_4t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    c.bench_function("concurrent_read_write_4t", |b| {
        b.iter(|| {
            let eval_writer = Arc::clone(&evaluator);
            let writer = thread::spawn(move || {
                let mut guard = eval_writer.lock().unwrap();
                guard.update_state(black_box(BENCH_CONFIG_ALT)).unwrap();
            });

            let readers: Vec<_> = (0..3)
                .map(|_| {
                    let eval = Arc::clone(&evaluator);
                    thread::spawn(move || {
                        let ctx = json!({});
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box("boolFlag"), black_box(&ctx))
                    })
                })
                .collect();

            writer.join().unwrap();
            for h in readers {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C7: 16 threads evaluating simple flag
// ---------------------------------------------------------------------------

/// C7: 16 threads concurrently evaluating a simple (static) flag.
///
/// Tests throughput saturation — at 16 threads the mutex contention dominates,
/// revealing the scalability ceiling of the current locking strategy.
fn concurrent_simple_16t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    c.bench_function("concurrent_simple_16t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..16)
                .map(|_| {
                    let eval = Arc::clone(&evaluator);
                    thread::spawn(move || {
                        let ctx = json!({});
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box("boolFlag"), black_box(&ctx))
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C8: 16 threads evaluating targeting flag
// ---------------------------------------------------------------------------

/// C8: 16 threads concurrently evaluating a flag with targeting rules.
///
/// Combines heavy mutex contention with per-evaluation rule processing,
/// measuring how targeting overhead compounds under high parallelism.
fn concurrent_targeting_16t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    c.bench_function("concurrent_targeting_16t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..16)
                .map(|i| {
                    let eval = Arc::clone(&evaluator);
                    thread::spawn(move || {
                        let role = if i % 2 == 0 { "admin" } else { "viewer" };
                        let ctx = json!({"role": role});
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box("targetedFlag"), black_box(&ctx))
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C9: Mixed workload - 16 threads with simple/targeting/disabled flags
// ---------------------------------------------------------------------------

/// C9: 16 threads with mixed workload (simple, targeting, and disabled flags).
///
/// Simulates a realistic high-load production scenario where threads evaluate
/// different flag types concurrently. The workload distribution cycles through
/// static, targeting, and disabled flags across all 16 threads.
fn concurrent_mixed_16t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    // Cycle through workload types across 16 threads:
    // simple, targeting(admin), disabled, targeting(viewer), repeat...
    let workload_defs: Vec<(&str, Value)> = vec![
        ("boolFlag", json!({})),
        ("targetedFlag", json!({"role": "admin"})),
        ("disabledFlag", json!({})),
        ("targetedFlag", json!({"role": "viewer"})),
    ];

    c.bench_function("concurrent_mixed_16t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..16)
                .map(|i| {
                    let eval = Arc::clone(&evaluator);
                    let (key, ctx) = workload_defs[i % workload_defs.len()].clone();
                    thread::spawn(move || {
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box(key), black_box(&ctx))
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

// ---------------------------------------------------------------------------
// C10: Read/write contention - 15 readers + 1 writer (16 threads total)
// ---------------------------------------------------------------------------

/// C10: Read/write contention at 16 threads — 15 evaluating while 1 updates state.
///
/// The writer thread alternates between two configurations on each iteration,
/// simulating periodic config refreshes under heavy parallel evaluation load.
/// This measures contention between readers and a writer at high thread counts.
fn concurrent_read_write_16t(c: &mut Criterion) {
    let evaluator = make_evaluator();

    c.bench_function("concurrent_read_write_16t", |b| {
        b.iter(|| {
            let eval_writer = Arc::clone(&evaluator);
            let writer = thread::spawn(move || {
                let mut guard = eval_writer.lock().unwrap();
                guard.update_state(black_box(BENCH_CONFIG_ALT)).unwrap();
            });

            let readers: Vec<_> = (0..15)
                .map(|_| {
                    let eval = Arc::clone(&evaluator);
                    thread::spawn(move || {
                        let ctx = json!({});
                        let guard = eval.lock().unwrap();
                        guard.evaluate_flag(black_box("boolFlag"), black_box(&ctx))
                    })
                })
                .collect();

            writer.join().unwrap();
            for h in readers {
                h.join().unwrap();
            }
        })
    });
}

criterion_group!(
    benches,
    concurrent_simple_1t,
    concurrent_simple_4t,
    concurrent_simple_8t,
    concurrent_targeting_4t,
    concurrent_mixed_4t,
    concurrent_read_write_4t,
    concurrent_simple_16t,
    concurrent_targeting_16t,
    concurrent_mixed_16t,
    concurrent_read_write_16t,
);
criterion_main!(benches);
