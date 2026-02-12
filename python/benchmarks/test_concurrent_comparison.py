"""3-way concurrent comparison: PyO3 vs WASM (wasmtime) vs panzi-json-logic.

Measures throughput, per-eval time, and memory under concurrent load.
- PyO3: Rust native bindings, releases GIL → true parallelism
- WASM: wasmtime-py, holds GIL + internal lock → serial
- panzi-json-logic: pure Python, GIL-bound → serial

Run with:
    pytest benchmarks/test_concurrent_comparison.py --benchmark-only -v
    python benchmarks/test_concurrent_comparison.py   # standalone with memory
"""

import concurrent.futures
import sys
import time
import tracemalloc

import pytest
from flagd_evaluator import FlagEvaluator
from json_logic import jsonLogic

sys.path.insert(0, ".")
from flagd_evaluator_wasm import WasmFlagEvaluator


# Flag configurations matching the Java/Go comparison benchmarks
SIMPLE_FLAG_CONFIG = {
    "flags": {
        "simple-bool": {
            "state": "ENABLED",
            "variants": {"on": True, "off": False},
            "defaultVariant": "on",
        }
    }
}

TARGETING_FLAG_CONFIG = {
    "flags": {
        "targeted-access": {
            "state": "ENABLED",
            "variants": {"denied": False, "granted": True},
            "defaultVariant": "denied",
            "targeting": {
                "if": [
                    {
                        "and": [
                            {"==": [{"var": "role"}, "admin"]},
                            {"in": [{"var": "tier"}, ["premium", "enterprise"]]},
                        ]
                    },
                    "granted",
                    None,
                ]
            },
        }
    }
}

TARGETING_RULE = {
    "if": [
        {
            "and": [
                {"==": [{"var": "role"}, "admin"]},
                {"in": [{"var": "tier"}, ["premium", "enterprise"]]},
            ]
        },
        "granted",
        None,
    ]
}

SMALL_CONTEXT = {
    "targetingKey": "user-123",
    "tier": "premium",
    "role": "admin",
    "region": "us-east",
    "score": 85,
}

LARGE_CONTEXT = {
    "targetingKey": "user-123",
    "tier": "premium",
    "role": "admin",
    "region": "us-east",
    "score": 85,
}
for _i in range(100):
    LARGE_CONTEXT[f"attr_{_i}"] = f"value_{_i}"

ITERATIONS_PER_WORKER = 200


# ---------------------------------------------------------------------------
# pytest-benchmark comparison tests
# ---------------------------------------------------------------------------


class TestConcurrentComparison:
    """3-way: PyO3 vs WASM vs panzi-json-logic under concurrency."""

    # -- Single-threaded baselines --

    def test_1t_pyo3_targeting(self, benchmark):
        """PyO3: single-threaded targeting evaluation."""
        ev = FlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)
        result = benchmark(ev.evaluate_bool, "targeted-access", SMALL_CONTEXT, False)
        assert result is True

    def test_1t_wasm_targeting(self, benchmark):
        """WASM: single-threaded targeting evaluation."""
        ev = WasmFlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)
        result = benchmark(ev.evaluate_bool, "targeted-access", SMALL_CONTEXT, False)
        assert result is True

    def test_1t_panzi_targeting(self, benchmark):
        """panzi-json-logic: single-threaded targeting evaluation."""
        result = benchmark(jsonLogic, TARGETING_RULE, SMALL_CONTEXT)
        assert result == "granted"

    # -- 4-thread concurrent targeting --

    def test_4t_pyo3_targeting(self, benchmark):
        """PyO3: 4 threads targeting (GIL released)."""
        ev = FlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                futures = [
                    pool.submit(_worker_pyo3, ev, ITERATIONS_PER_WORKER)
                    for _ in range(4)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    def test_4t_wasm_targeting(self, benchmark):
        """WASM: 4 threads targeting (lock-serialized)."""
        ev = WasmFlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                futures = [
                    pool.submit(_worker_wasm, ev, ITERATIONS_PER_WORKER)
                    for _ in range(4)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    def test_4t_panzi_targeting(self, benchmark):
        """panzi-json-logic: 4 threads targeting (GIL-bound)."""

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                futures = [
                    pool.submit(_worker_panzi, ITERATIONS_PER_WORKER)
                    for _ in range(4)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    # -- 16-thread concurrent targeting --

    def test_16t_pyo3_targeting(self, benchmark):
        """PyO3: 16 threads targeting (GIL released)."""
        ev = FlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
                futures = [
                    pool.submit(_worker_pyo3, ev, ITERATIONS_PER_WORKER)
                    for _ in range(16)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    def test_16t_wasm_targeting(self, benchmark):
        """WASM: 16 threads targeting (lock-serialized)."""
        ev = WasmFlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
                futures = [
                    pool.submit(_worker_wasm, ev, ITERATIONS_PER_WORKER)
                    for _ in range(16)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    def test_16t_panzi_targeting(self, benchmark):
        """panzi-json-logic: 16 threads targeting (GIL-bound)."""

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
                futures = [
                    pool.submit(_worker_panzi, ITERATIONS_PER_WORKER)
                    for _ in range(16)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    # -- 4-thread large context --

    def test_4t_pyo3_largeCtx(self, benchmark):
        """PyO3: 4 threads, large context (context filtering)."""
        ev = FlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                futures = [
                    pool.submit(_worker_pyo3_large, ev, ITERATIONS_PER_WORKER)
                    for _ in range(4)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    def test_4t_wasm_largeCtx(self, benchmark):
        """WASM: 4 threads, large context (context filtering)."""
        ev = WasmFlagEvaluator()
        ev.update_state(TARGETING_FLAG_CONFIG)

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                futures = [
                    pool.submit(_worker_wasm_large, ev, ITERATIONS_PER_WORKER)
                    for _ in range(4)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)

    def test_4t_panzi_largeCtx(self, benchmark):
        """panzi-json-logic: 4 threads, large context (full context)."""

        def workload():
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                futures = [
                    pool.submit(_worker_panzi_large, ITERATIONS_PER_WORKER)
                    for _ in range(4)
                ]
                for f in futures:
                    f.result()

        benchmark.pedantic(workload, rounds=5, warmup_rounds=1)


# ---------------------------------------------------------------------------
# Worker functions
# ---------------------------------------------------------------------------


def _worker_pyo3(ev, n):
    for _ in range(n):
        ev.evaluate_bool("targeted-access", SMALL_CONTEXT, False)


def _worker_wasm(ev, n):
    for _ in range(n):
        ev.evaluate_bool("targeted-access", SMALL_CONTEXT, False)


def _worker_panzi(n):
    for _ in range(n):
        jsonLogic(TARGETING_RULE, SMALL_CONTEXT)


def _worker_pyo3_large(ev, n):
    for _ in range(n):
        ev.evaluate_bool("targeted-access", LARGE_CONTEXT, False)


def _worker_wasm_large(ev, n):
    for _ in range(n):
        ev.evaluate_bool("targeted-access", LARGE_CONTEXT, False)


def _worker_panzi_large(n):
    for _ in range(n):
        jsonLogic(TARGETING_RULE, LARGE_CONTEXT)


def _worker_pyo3_simple(ev, n):
    for _ in range(n):
        ev.evaluate_bool("simple-bool", {}, False)


def _worker_wasm_simple(ev, n):
    for _ in range(n):
        ev.evaluate_bool("simple-bool", {}, False)


def _worker_panzi_simple(n):
    for _ in range(n):
        jsonLogic(True, {})


# ---------------------------------------------------------------------------
# Standalone runner with memory measurement
# ---------------------------------------------------------------------------


def _measure(label, func, threads, iterations):
    """Measure throughput, per-eval time, and peak memory."""
    tracemalloc.start()
    start = time.perf_counter()
    func()
    elapsed = time.perf_counter() - start
    current, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    total_ops = threads * iterations
    throughput = total_ops / elapsed
    per_eval_ns = (elapsed / total_ops) * 1e9

    return {
        "label": label,
        "threads": threads,
        "total_ops": total_ops,
        "elapsed_s": elapsed,
        "throughput_ops_s": throughput,
        "per_eval_ns": per_eval_ns,
        "peak_memory_kb": peak / 1024,
    }


def main():
    """Run standalone 3-way comparison with memory tracking."""
    results = []

    # Setup evaluators
    ev_pyo3 = FlagEvaluator()
    ev_pyo3.update_state(TARGETING_FLAG_CONFIG)
    ev_wasm = WasmFlagEvaluator()
    ev_wasm.update_state(TARGETING_FLAG_CONFIG)

    ev_pyo3_simple = FlagEvaluator()
    ev_pyo3_simple.update_state(SIMPLE_FLAG_CONFIG)
    ev_wasm_simple = WasmFlagEvaluator()
    ev_wasm_simple.update_state(SIMPLE_FLAG_CONFIG)

    iters = 500

    for threads in [1, 4, 16]:
        # Targeting: PyO3
        def f1(ev=ev_pyo3, t=threads, n=iters):
            with concurrent.futures.ThreadPoolExecutor(max_workers=t) as pool:
                fs = [pool.submit(_worker_pyo3, ev, n) for _ in range(t)]
                for f in fs:
                    f.result()

        results.append(_measure("PyO3 targeting", f1, threads, iters))

        # Targeting: WASM
        def f2(ev=ev_wasm, t=threads, n=iters):
            with concurrent.futures.ThreadPoolExecutor(max_workers=t) as pool:
                fs = [pool.submit(_worker_wasm, ev, n) for _ in range(t)]
                for f in fs:
                    f.result()

        results.append(_measure("WASM targeting", f2, threads, iters))

        # Targeting: panzi
        def f3(t=threads, n=iters):
            with concurrent.futures.ThreadPoolExecutor(max_workers=t) as pool:
                fs = [pool.submit(_worker_panzi, n) for _ in range(t)]
                for f in fs:
                    f.result()

        results.append(_measure("panzi targeting", f3, threads, iters))

        # Simple: PyO3
        def f4(ev=ev_pyo3_simple, t=threads, n=iters):
            with concurrent.futures.ThreadPoolExecutor(max_workers=t) as pool:
                fs = [pool.submit(_worker_pyo3_simple, ev, n) for _ in range(t)]
                for f in fs:
                    f.result()

        results.append(_measure("PyO3 simple", f4, threads, iters))

        # Simple: WASM
        def f5(ev=ev_wasm_simple, t=threads, n=iters):
            with concurrent.futures.ThreadPoolExecutor(max_workers=t) as pool:
                fs = [pool.submit(_worker_wasm_simple, ev, n) for _ in range(t)]
                for f in fs:
                    f.result()

        results.append(_measure("WASM simple", f5, threads, iters))

        # Simple: panzi
        def f6(t=threads, n=iters):
            with concurrent.futures.ThreadPoolExecutor(max_workers=t) as pool:
                fs = [pool.submit(_worker_panzi_simple, n) for _ in range(t)]
                for f in fs:
                    f.result()

        results.append(_measure("panzi simple", f6, threads, iters))

    # Large context at 4 threads
    def fl1(ev=ev_pyo3, n=iters):
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            fs = [pool.submit(_worker_pyo3_large, ev, n) for _ in range(4)]
            for f in fs:
                f.result()

    results.append(_measure("PyO3 large ctx", fl1, 4, iters))

    def fl2(ev=ev_wasm, n=iters):
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            fs = [pool.submit(_worker_wasm_large, ev, n) for _ in range(4)]
            for f in fs:
                f.result()

    results.append(_measure("WASM large ctx", fl2, 4, iters))

    def fl3(n=iters):
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            fs = [pool.submit(_worker_panzi_large, n) for _ in range(4)]
            for f in fs:
                f.result()

    results.append(_measure("panzi large ctx", fl3, 4, iters))

    # Print results
    print("\n" + "=" * 105)
    print(
        "PYTHON 3-WAY COMPARISON: PyO3 (Rust native) vs WASM (wasmtime) vs panzi-json-logic"
    )
    print("=" * 105)
    print(
        f"{'Scenario':<25} {'Threads':>7} {'Ops':>8} {'Throughput':>15} "
        f"{'ns/eval':>12} {'Peak Mem KB':>12}"
    )
    print("-" * 105)

    prev_threads = None
    for r in results:
        if prev_threads is not None and r["threads"] != prev_threads:
            print()
        prev_threads = r["threads"]
        print(
            f"{r['label']:<25} {r['threads']:>7} {r['total_ops']:>8} "
            f"{r['throughput_ops_s']:>12,.0f} ops/s "
            f"{r['per_eval_ns']:>9,.0f} ns "
            f"{r['peak_memory_kb']:>9,.1f} KB"
        )
    print("=" * 105)


if __name__ == "__main__":
    main()
