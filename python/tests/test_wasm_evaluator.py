"""Tests for the WASM-based flag evaluator (wasmtime).

Mirrors test_basic.py and test_flag_evaluation.py to ensure API parity
between the PyO3 (FlagEvaluator) and WASM (WasmFlagEvaluator) implementations.
"""

import pytest

from flagd_evaluator_wasm import WasmFlagEvaluator


# ---------------------------------------------------------------------------
# Basic tests (mirrors test_basic.py)
# ---------------------------------------------------------------------------


class TestBasic:
    def test_init(self):
        evaluator = WasmFlagEvaluator()
        assert evaluator is not None
        evaluator.close()

    def test_update_state(self):
        evaluator = WasmFlagEvaluator()
        result = evaluator.update_state({
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                }
            }
        })
        assert result["success"] is True
        evaluator.close()

    def test_bool_flag(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                }
            }
        })
        result = evaluator.evaluate_bool("boolFlag", {}, False)
        assert result is True
        evaluator.close()

    def test_string_flag(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {"red": "color-red", "blue": "color-blue"},
                    "defaultVariant": "red",
                }
            }
        })
        result = evaluator.evaluate_string("stringFlag", {}, "default")
        assert result == "color-red"
        evaluator.close()

    def test_int_flag(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {"small": 10, "large": 100},
                    "defaultVariant": "small",
                }
            }
        })
        result = evaluator.evaluate_int("intFlag", {}, 0)
        assert result == 10
        evaluator.close()

    def test_float_flag(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {"low": 1.5, "high": 9.9},
                    "defaultVariant": "low",
                }
            }
        })
        result = evaluator.evaluate_float("floatFlag", {}, 0.0)
        assert result == 1.5
        evaluator.close()

    def test_flag_not_found(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "existingFlag": {
                    "state": "ENABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                }
            }
        })
        # Should return default value for non-existent flag
        result = evaluator.evaluate_bool("nonExistentFlag", {}, False)
        assert result is False
        result2 = evaluator.evaluate_string("nonExistentFlag", {}, "fallback")
        assert result2 == "fallback"
        evaluator.close()

    def test_no_state(self):
        evaluator = WasmFlagEvaluator()
        result = evaluator.evaluate_bool("myFlag", {}, False)
        assert result is False
        result2 = evaluator.evaluate_bool("myFlag", {}, True)
        assert result2 is True
        evaluator.close()


# ---------------------------------------------------------------------------
# Flag evaluation tests (mirrors test_flag_evaluation.py)
# ---------------------------------------------------------------------------


class TestFlagEvaluation:
    def test_targeting_rule(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "targetedFlag": {
                    "state": "ENABLED",
                    "variants": {"admin": "admin-view", "user": "user-view"},
                    "defaultVariant": "user",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "role"}, "admin"]},
                            "admin",
                            "user",
                        ]
                    },
                }
            }
        })

        result = evaluator.evaluate("targetedFlag", {"role": "admin"})
        assert result["value"] == "admin-view"
        assert result["variant"] == "admin"

        result2 = evaluator.evaluate("targetedFlag", {"role": "user"})
        assert result2["value"] == "user-view"
        assert result2["variant"] == "user"
        evaluator.close()

    def test_disabled_flag(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "disabledFlag": {
                    "state": "DISABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                }
            }
        })
        result = evaluator.evaluate("disabledFlag", {})
        assert result["reason"] == "DISABLED"
        evaluator.close()

    def test_multiple_flags(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                },
                "flag2": {
                    "state": "ENABLED",
                    "variants": {"red": "color-red", "blue": "color-blue"},
                    "defaultVariant": "blue",
                },
                "flag3": {
                    "state": "ENABLED",
                    "variants": {"small": 10, "large": 100},
                    "defaultVariant": "large",
                },
            }
        })
        assert evaluator.evaluate_bool("flag1", {}, False) is True
        assert evaluator.evaluate_string("flag2", {}, "default") == "color-blue"
        assert evaluator.evaluate_int("flag3", {}, 0) == 100
        evaluator.close()

    def test_evaluate_full_result(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                }
            }
        })
        result = evaluator.evaluate("testFlag", {})
        assert "value" in result
        assert "variant" in result
        assert "reason" in result
        assert result["value"] is True
        assert result["variant"] == "on"
        assert result["reason"] in ["STATIC", "TARGETING_MATCH", "DEFAULT"]
        evaluator.close()

    def test_fractional_targeting(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "abTestFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "control": {"color": "blue", "size": "medium"},
                        "treatment": {"color": "green", "size": "large"},
                    },
                    "defaultVariant": "control",
                    "targeting": {
                        "fractional": [
                            {"var": "userId"},
                            ["control", 50],
                            ["treatment", 50],
                        ]
                    },
                }
            }
        })
        result = evaluator.evaluate("abTestFlag", {"userId": "user123"})
        assert result["variant"] in ["control", "treatment"]
        evaluator.close()

    def test_context_enrichment(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "keyFlag": {
                    "state": "ENABLED",
                    "variants": {"match": "matched", "no": "nope"},
                    "defaultVariant": "no",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "targetingKey"}, "user-abc"]},
                            "match",
                            "no",
                        ]
                    },
                }
            }
        })
        result = evaluator.evaluate("keyFlag", {"targetingKey": "user-abc"})
        assert result["value"] == "matched"
        evaluator.close()

    def test_complex_targeting(self):
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "complexFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "premium": "premium-feature",
                        "basic": "basic-feature",
                    },
                    "defaultVariant": "basic",
                    "targeting": {
                        "if": [
                            {
                                "and": [
                                    {">": [{"var": "age"}, 18]},
                                    {
                                        "starts_with": [
                                            {"var": "email"},
                                            "premium@",
                                        ]
                                    },
                                ]
                            },
                            "premium",
                            "basic",
                        ]
                    },
                }
            }
        })

        result = evaluator.evaluate(
            "complexFlag", {"age": 25, "email": "premium@example.com"}
        )
        assert result["value"] == "premium-feature"

        result2 = evaluator.evaluate(
            "complexFlag", {"age": 25, "email": "user@example.com"}
        )
        assert result2["value"] == "basic-feature"

        result3 = evaluator.evaluate(
            "complexFlag", {"age": 16, "email": "premium@example.com"}
        )
        assert result3["value"] == "basic-feature"
        evaluator.close()


# ---------------------------------------------------------------------------
# Cache / optimization tests
# ---------------------------------------------------------------------------


class TestOptimizations:
    def test_pre_evaluated_cache(self):
        """Static flags should be served from the pre-evaluated cache."""
        evaluator = WasmFlagEvaluator()
        result = evaluator.update_state({
            "flags": {
                "staticFlag": {
                    "state": "ENABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                }
            }
        })
        # Pre-evaluated cache should contain static flags
        assert "preEvaluated" in result
        assert "staticFlag" in result["preEvaluated"]

        # Evaluation should still return correct value
        value = evaluator.evaluate_bool("staticFlag", {}, False)
        assert value is True
        evaluator.close()

    def test_required_context_keys(self):
        """Targeting flags should have required context keys populated."""
        evaluator = WasmFlagEvaluator()
        result = evaluator.update_state({
            "flags": {
                "targetedFlag": {
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
                }
            }
        })
        assert "requiredContextKeys" in result
        assert "targetedFlag" in result["requiredContextKeys"]
        keys = result["requiredContextKeys"]["targetedFlag"]
        assert "tier" in keys
        evaluator.close()

    def test_flag_indices(self):
        """Flag indices should be returned by update_state."""
        evaluator = WasmFlagEvaluator()
        result = evaluator.update_state({
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "variants": {"on": True, "off": False},
                    "defaultVariant": "on",
                },
                "flag2": {
                    "state": "ENABLED",
                    "variants": {"a": "x", "b": "y"},
                    "defaultVariant": "a",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "role"}, "admin"]},
                            "a",
                            "b",
                        ]
                    },
                },
            }
        })
        assert "flagIndices" in result
        evaluator.close()

    def test_close(self):
        """Close should work without error."""
        evaluator = WasmFlagEvaluator()
        evaluator.update_state({
            "flags": {
                "flag": {
                    "state": "ENABLED",
                    "variants": {"on": True},
                    "defaultVariant": "on",
                }
            }
        })
        evaluator.close()
        # Double-close should also be safe
        evaluator.close()
