"""Head-to-head benchmarks: PyO3 vs WASM vs Pure Python (json-logic).

Run with: pytest benchmarks/test_wasm_comparison.py --benchmark-only -v

Three implementations compared:
  - PyO3:   Rust native bindings via pyo3/maturin
  - WASM:   wasmtime-py driving the shared flagd-evaluator.wasm binary
  - Python: panzi-json-logic (what the flagd Python provider uses today)
"""

import time

import pytest
from flagd_evaluator import FlagEvaluator
from json_logic.apply import apply as jsonLogic

from flagd_evaluator_wasm import WasmFlagEvaluator


# ---------------------------------------------------------------------------
# Pure Python flag evaluator — simulates what flagd Python provider does
# ---------------------------------------------------------------------------


class PythonFlagEvaluator:
    """Minimal flag evaluator using panzi-json-logic.

    No caching, no context filtering — just dict lookups + jsonLogic().
    This represents the realistic baseline for a pure-Python provider.
    """

    def __init__(self):
        self._flags = {}

    def update_state(self, config):
        self._flags = config.get("flags", {})

    def evaluate(self, flag_key, context):
        flag = self._flags.get(flag_key)
        if flag is None:
            return {
                "value": None,
                "variant": "",
                "reason": "FLAG_NOT_FOUND",
                "errorCode": "FLAG_NOT_FOUND",
            }

        if flag.get("state") == "DISABLED":
            default_variant = flag["defaultVariant"]
            return {
                "value": flag["variants"][default_variant],
                "variant": default_variant,
                "reason": "DISABLED",
            }

        targeting = flag.get("targeting")
        if targeting:
            # Enrich context like the WASM evaluator does
            ctx = dict(context)
            ctx.setdefault("targetingKey", "")
            ctx["$flagd"] = {
                "flagKey": flag_key,
                "timestamp": int(time.time()),
            }
            variant = jsonLogic(targeting, ctx)
            if isinstance(variant, str) and variant in flag["variants"]:
                return {
                    "value": flag["variants"][variant],
                    "variant": variant,
                    "reason": "TARGETING_MATCH",
                }

        default_variant = flag["defaultVariant"]
        return {
            "value": flag["variants"][default_variant],
            "variant": default_variant,
            "reason": "STATIC",
        }

    def evaluate_bool(self, flag_key, context, default):
        result = self.evaluate(flag_key, context)
        if result.get("errorCode"):
            return default
        value = result.get("value")
        return value if isinstance(value, bool) else default

    def evaluate_string(self, flag_key, context, default):
        result = self.evaluate(flag_key, context)
        if result.get("errorCode"):
            return default
        value = result.get("value")
        return value if isinstance(value, str) else default

    def evaluate_int(self, flag_key, context, default):
        result = self.evaluate(flag_key, context)
        if result.get("errorCode"):
            return default
        value = result.get("value")
        return int(value) if isinstance(value, (int, float)) else default

    def evaluate_float(self, flag_key, context, default):
        result = self.evaluate(flag_key, context)
        if result.get("errorCode"):
            return default
        value = result.get("value")
        return float(value) if isinstance(value, (int, float)) else default


# ---------------------------------------------------------------------------
# Shared flag configuration
# ---------------------------------------------------------------------------


def _build_flag_config():
    return {
        "flags": {
            "simple-bool": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on",
            },
            "targeted-bool": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "off",
                "targeting": {
                    "if": [
                        {"==": [{"var": "tier"}, "premium"]},
                        "on",
                        "off",
                    ]
                },
            },
            "disabled-flag": {
                "state": "DISABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on",
            },
            "complex-targeting": {
                "state": "ENABLED",
                "defaultVariant": "basic",
                "variants": {
                    "premium": "premium-tier",
                    "standard": "standard-tier",
                    "basic": "basic-tier",
                },
                "targeting": {
                    "if": [
                        {
                            "and": [
                                {"==": [{"var": "tier"}, "premium"]},
                                {">": [{"var": "score"}, 90]},
                            ]
                        },
                        "premium",
                        {
                            "if": [
                                {
                                    "or": [
                                        {"==": [{"var": "tier"}, "standard"]},
                                        {">": [{"var": "score"}, 50]},
                                    ]
                                },
                                "standard",
                                "basic",
                            ]
                        },
                    ]
                },
            },
        }
    }


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def pyo3_evaluator():
    ev = FlagEvaluator()
    ev.update_state(_build_flag_config())
    return ev


@pytest.fixture
def wasm_evaluator():
    ev = WasmFlagEvaluator()
    ev.update_state(_build_flag_config())
    return ev


@pytest.fixture
def python_evaluator():
    ev = PythonFlagEvaluator()
    ev.update_state(_build_flag_config())
    return ev


@pytest.fixture
def small_context():
    return {
        "targetingKey": "user-123",
        "tier": "premium",
        "role": "admin",
        "region": "us-east",
        "score": 85,
    }


@pytest.fixture
def large_context():
    ctx = {
        "targetingKey": "user-bench-12345",
        "tier": "premium",
        "segment": "beta",
        "email": "admin@corp.example.com",
        "appVersion": "2.5.1",
        "role": "admin",
        "country": "US",
        "locale": "en-US",
        "platform": "linux",
        "deviceType": "desktop",
    }
    for i in range(100):
        ctx[f"attr_{i}"] = f"value_{i}"
    return ctx


@pytest.fixture
def xlarge_context():
    ctx = {
        "targetingKey": "user-bench-99999",
        "tier": "premium",
        "score": 85,
    }
    for i in range(1000):
        ctx[f"attr_{i}"] = f"value_{i}"
    return ctx


# ---------------------------------------------------------------------------
# E1: Static flag, empty context
# ---------------------------------------------------------------------------


class TestE1StaticEmpty:
    def test_pyo3(self, benchmark, pyo3_evaluator):
        """PyO3: static flag, empty context."""
        result = benchmark(pyo3_evaluator.evaluate_bool, "simple-bool", {}, False)
        assert result is True

    def test_wasm(self, benchmark, wasm_evaluator):
        """WASM: static flag, empty context (pre-eval cache)."""
        result = benchmark(wasm_evaluator.evaluate_bool, "simple-bool", {}, False)
        assert result is True

    def test_python(self, benchmark, python_evaluator):
        """Python: static flag, empty context (dict lookup)."""
        result = benchmark(python_evaluator.evaluate_bool, "simple-bool", {}, False)
        assert result is True


# ---------------------------------------------------------------------------
# E2: Static flag, small context
# ---------------------------------------------------------------------------


class TestE2StaticSmall:
    def test_pyo3(self, benchmark, pyo3_evaluator, small_context):
        """PyO3: static flag, small context."""
        result = benchmark(
            pyo3_evaluator.evaluate_bool, "simple-bool", small_context, False
        )
        assert result is True

    def test_wasm(self, benchmark, wasm_evaluator, small_context):
        """WASM: static flag, small context (pre-eval cache, no serialization)."""
        result = benchmark(
            wasm_evaluator.evaluate_bool, "simple-bool", small_context, False
        )
        assert result is True

    def test_python(self, benchmark, python_evaluator, small_context):
        """Python: static flag, small context (dict lookup, no jsonLogic)."""
        result = benchmark(
            python_evaluator.evaluate_bool, "simple-bool", small_context, False
        )
        assert result is True


# ---------------------------------------------------------------------------
# E3: Static flag, large context
# ---------------------------------------------------------------------------


class TestE3StaticLarge:
    def test_pyo3(self, benchmark, pyo3_evaluator, large_context):
        """PyO3: static flag, large context (dict crosses PyO3 boundary)."""
        result = benchmark(
            pyo3_evaluator.evaluate_bool, "simple-bool", large_context, False
        )
        assert result is True

    def test_wasm(self, benchmark, wasm_evaluator, large_context):
        """WASM: static flag, large context (pre-eval cache, no serialization)."""
        result = benchmark(
            wasm_evaluator.evaluate_bool, "simple-bool", large_context, False
        )
        assert result is True

    def test_python(self, benchmark, python_evaluator, large_context):
        """Python: static flag, large context (dict lookup, no jsonLogic)."""
        result = benchmark(
            python_evaluator.evaluate_bool, "simple-bool", large_context, False
        )
        assert result is True


# ---------------------------------------------------------------------------
# E4: Targeting, small context
# ---------------------------------------------------------------------------


class TestE4TargetingSmall:
    def test_pyo3(self, benchmark, pyo3_evaluator, small_context):
        """PyO3: targeting flag, small context."""
        result = benchmark(
            pyo3_evaluator.evaluate_bool, "targeted-bool", small_context, False
        )
        assert result is True

    def test_wasm(self, benchmark, wasm_evaluator, small_context):
        """WASM: targeting flag, small context (filtered serialization)."""
        result = benchmark(
            wasm_evaluator.evaluate_bool, "targeted-bool", small_context, False
        )
        assert result is True

    def test_python(self, benchmark, python_evaluator, small_context):
        """Python: targeting flag, small context (jsonLogic eval)."""
        result = benchmark(
            python_evaluator.evaluate_bool, "targeted-bool", small_context, False
        )
        assert result is True


# ---------------------------------------------------------------------------
# E5: Targeting, large context (100+ attrs)
# ---------------------------------------------------------------------------


class TestE5TargetingLarge:
    def test_pyo3(self, benchmark, pyo3_evaluator, large_context):
        """PyO3: targeting flag, large context (full dict crosses boundary)."""
        result = benchmark(
            pyo3_evaluator.evaluate_bool, "targeted-bool", large_context, False
        )
        assert result is True

    def test_wasm(self, benchmark, wasm_evaluator, large_context):
        """WASM: targeting flag, large context (filter 110→2 keys)."""
        result = benchmark(
            wasm_evaluator.evaluate_bool, "targeted-bool", large_context, False
        )
        assert result is True

    def test_python(self, benchmark, python_evaluator, large_context):
        """Python: targeting flag, large context (jsonLogic gets full dict)."""
        result = benchmark(
            python_evaluator.evaluate_bool, "targeted-bool", large_context, False
        )
        assert result is True


# ---------------------------------------------------------------------------
# E5b: Targeting, xlarge context (1000+ attrs)
# ---------------------------------------------------------------------------


class TestE5bTargetingXLarge:
    def test_pyo3(self, benchmark, pyo3_evaluator, xlarge_context):
        """PyO3: targeting flag, 1000+ attr context."""
        result = benchmark(
            pyo3_evaluator.evaluate_bool, "targeted-bool", xlarge_context, False
        )
        assert result is True

    def test_wasm(self, benchmark, wasm_evaluator, xlarge_context):
        """WASM: targeting flag, 1000+ attr context (filter 1003→2 keys)."""
        result = benchmark(
            wasm_evaluator.evaluate_bool, "targeted-bool", xlarge_context, False
        )
        assert result is True

    def test_python(self, benchmark, python_evaluator, xlarge_context):
        """Python: targeting flag, 1000+ attr context (jsonLogic gets full dict)."""
        result = benchmark(
            python_evaluator.evaluate_bool, "targeted-bool", xlarge_context, False
        )
        assert result is True


# ---------------------------------------------------------------------------
# E6: Complex targeting, small context
# ---------------------------------------------------------------------------


class TestE6ComplexSmall:
    def test_pyo3(self, benchmark, pyo3_evaluator, small_context):
        """PyO3: complex targeting, small context."""
        result = benchmark(
            pyo3_evaluator.evaluate_string,
            "complex-targeting",
            small_context,
            "fallback",
        )
        assert result == "standard-tier"

    def test_wasm(self, benchmark, wasm_evaluator, small_context):
        """WASM: complex targeting, small context."""
        result = benchmark(
            wasm_evaluator.evaluate_string,
            "complex-targeting",
            small_context,
            "fallback",
        )
        assert result == "standard-tier"

    def test_python(self, benchmark, python_evaluator, small_context):
        """Python: complex targeting, small context (nested jsonLogic)."""
        result = benchmark(
            python_evaluator.evaluate_string,
            "complex-targeting",
            small_context,
            "fallback",
        )
        assert result == "standard-tier"


# ---------------------------------------------------------------------------
# E10: Disabled flag (cache)
# ---------------------------------------------------------------------------


class TestE10Disabled:
    def test_pyo3(self, benchmark, pyo3_evaluator):
        """PyO3: disabled flag."""
        result = benchmark(pyo3_evaluator.evaluate, "disabled-flag", {})
        assert result["reason"] == "DISABLED"

    def test_wasm(self, benchmark, wasm_evaluator):
        """WASM: disabled flag (pre-eval cache)."""
        result = benchmark(wasm_evaluator.evaluate, "disabled-flag", {})
        assert result["reason"] == "DISABLED"

    def test_python(self, benchmark, python_evaluator):
        """Python: disabled flag (dict lookup, early exit)."""
        result = benchmark(python_evaluator.evaluate, "disabled-flag", {})
        assert result["reason"] == "DISABLED"
