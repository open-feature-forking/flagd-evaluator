"""Benchmark suite for flagd-evaluator Python bindings.

Uses pytest-benchmark for statistically rigorous performance measurement.
Run with: pytest benchmarks/ --benchmark-only -v
"""

import concurrent.futures

import pytest
from flagd_evaluator import FlagEvaluator


# ---------------------------------------------------------------------------
# Evaluation benchmarks
# ---------------------------------------------------------------------------


class TestEvaluationBenchmarks:
    """Benchmarks for core flag evaluation across different types."""

    def test_bench_evaluate_bool_simple(self, benchmark, evaluator):
        """Boolean flag with no targeting rules (STATIC resolution)."""
        result = benchmark(evaluator.evaluate_bool, "simple-bool", {}, False)
        assert result is True

    def test_bench_evaluate_bool_targeting_match(self, benchmark, evaluator):
        """Boolean flag with targeting rule that matches."""
        ctx = {"tier": "premium"}
        result = benchmark(evaluator.evaluate_bool, "targeted-bool", ctx, False)
        assert result is True

    def test_bench_evaluate_bool_targeting_no_match(self, benchmark, evaluator):
        """Boolean flag with targeting rule that does not match."""
        ctx = {"tier": "free"}
        result = benchmark(evaluator.evaluate_bool, "targeted-bool", ctx, False)
        assert result is False

    def test_bench_evaluate_string(self, benchmark, evaluator):
        """String flag evaluation with targeting."""
        ctx = {"segment": "beta"}
        result = benchmark(evaluator.evaluate_string, "string-flag", ctx, "fallback")
        assert result == "Welcome to our new experience!"

    def test_bench_evaluate_int(self, benchmark, evaluator):
        """Integer flag evaluation (STATIC)."""
        result = benchmark(evaluator.evaluate_int, "int-flag", {}, 0)
        assert result == 50

    def test_bench_evaluate_float(self, benchmark, evaluator):
        """Float flag evaluation (STATIC)."""
        result = benchmark(evaluator.evaluate_float, "float-flag", {}, 0.0)
        assert result == 0.5

    def test_bench_evaluate_object(self, benchmark, evaluator):
        """Object/struct flag evaluation via generic evaluate()."""
        result = benchmark(evaluator.evaluate, "object-flag", {})
        assert result["value"]["color"] == "blue"
        assert "search" in result["value"]["features"]

    def test_bench_evaluate_with_large_context(self, benchmark, evaluator, large_context):
        """Boolean flag evaluation with 100+ attributes in context."""
        result = benchmark(evaluator.evaluate_bool, "targeted-bool", large_context, False)
        assert result is True


# ---------------------------------------------------------------------------
# Custom operator benchmarks
# ---------------------------------------------------------------------------


class TestCustomOperatorBenchmarks:
    """Benchmarks for flagd custom JSON Logic operators."""

    def test_bench_fractional_operator(self, benchmark, evaluator):
        """Fractional bucketing operator (MurmurHash3-based)."""
        ctx = {"targetingKey": "user-abc-123"}
        result = benchmark(evaluator.evaluate_string, "fractional-flag", ctx, "fallback")
        assert result in [
            "control-experience",
            "treatment-a-experience",
            "treatment-b-experience",
        ]

    def test_bench_semver_operator(self, benchmark, evaluator):
        """Semantic version comparison operator."""
        ctx = {"appVersion": "2.5.1"}
        result = benchmark(evaluator.evaluate_bool, "semver-flag", ctx, False)
        assert result is True

    def test_bench_starts_with_operator(self, benchmark, evaluator):
        """String prefix matching operator."""
        ctx = {"email": "admin@example.com"}
        result = benchmark(
            evaluator.evaluate_string, "starts-with-flag", ctx, "fallback"
        )
        assert result == "internal-access"

    def test_bench_ends_with_operator(self, benchmark, evaluator):
        """String suffix matching operator."""
        ctx = {"email": "user@corp.example.com"}
        result = benchmark(
            evaluator.evaluate_string, "ends-with-flag", ctx, "fallback"
        )
        assert result == "corporate-plan"


# ---------------------------------------------------------------------------
# State management benchmarks
# ---------------------------------------------------------------------------


def _generate_flags(n):
    """Generate n flag definitions for state-update benchmarks."""
    flags = {}
    for i in range(n):
        flags[f"bench-flag-{i}"] = {
            "state": "ENABLED",
            "variants": {"on": True, "off": False},
            "defaultVariant": "on",
        }
    return {"flags": flags}


class TestStateManagementBenchmarks:
    """Benchmarks for flag configuration state updates."""

    def test_bench_update_state_small(self, benchmark):
        """Update state with 5 flags."""
        config = _generate_flags(5)
        evaluator = FlagEvaluator()

        def run():
            evaluator.update_state(config)

        result = benchmark(run)

    def test_bench_update_state_medium(self, benchmark):
        """Update state with 50 flags."""
        config = _generate_flags(50)
        evaluator = FlagEvaluator()

        def run():
            evaluator.update_state(config)

        result = benchmark(run)

    def test_bench_update_state_large(self, benchmark):
        """Update state with 200 flags."""
        config = _generate_flags(200)
        evaluator = FlagEvaluator()

        def run():
            evaluator.update_state(config)

        result = benchmark(run)

    def test_bench_update_state_no_change(self, benchmark, flag_config):
        """Re-apply the same config (no actual changes)."""
        evaluator = FlagEvaluator()
        evaluator.update_state(flag_config)

        result = benchmark(evaluator.update_state, flag_config)


# ---------------------------------------------------------------------------
# Concurrent benchmarks
# ---------------------------------------------------------------------------


class TestConcurrentBenchmarks:
    """Benchmarks for multi-threaded flag evaluation."""

    def test_bench_concurrent_evaluate(self, benchmark, flag_config):
        """Evaluate flags concurrently with 4 worker threads."""
        iterations_per_worker = 50

        def concurrent_workload():
            # Each worker gets its own evaluator (PyO3 classes are not
            # automatically Send+Sync across Python threads without the GIL,
            # but the GIL ensures safety here).
            evaluator = FlagEvaluator()
            evaluator.update_state(flag_config)

            def worker():
                for _ in range(iterations_per_worker):
                    evaluator.evaluate_bool("simple-bool", {}, False)
                    evaluator.evaluate_bool(
                        "targeted-bool", {"tier": "premium"}, False
                    )
                    evaluator.evaluate_string(
                        "fractional-flag", {"targetingKey": "user-1"}, ""
                    )

            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                futures = [pool.submit(worker) for _ in range(4)]
                for f in futures:
                    f.result()

        benchmark.pedantic(concurrent_workload, rounds=5, warmup_rounds=1)


# ---------------------------------------------------------------------------
# Comparison benchmark (optional -- skips if library not installed)
# ---------------------------------------------------------------------------


class TestComparisonBenchmarks:
    """Compare native flagd-evaluator against a pure-Python JSON Logic lib."""

    def test_bench_native_json_logic(self, benchmark):
        """Baseline: evaluate a simple rule with the native evaluator."""
        from flagd_evaluator import evaluate_targeting

        targeting = {"==": [{"var": "tier"}, "premium"]}
        context = {"tier": "premium"}
        result = benchmark(evaluate_targeting, targeting, context)
        assert result["success"] is True
        assert result["result"] is True

    def test_bench_pure_python_json_logic(self, benchmark):
        """Compare: evaluate the same rule with json-logic-utils (pure Python)."""
        json_logic_utils = pytest.importorskip("json_logic_utils")

        rule = {"==": [{"var": "tier"}, "premium"]}
        data = {"tier": "premium"}
        result = benchmark(json_logic_utils.jsonLogic, rule, data)
        assert result is True
