"""Basic tests for flagd_evaluator Python bindings."""

import pytest


def test_flag_evaluator_init():
    """Test FlagEvaluator initialization."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    assert evaluator is not None


def test_flag_evaluator_update_state():
    """Test FlagEvaluator state update."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    result = evaluator.update_state({
        "flags": {
            "myFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })
    assert result["success"] is True


def test_flag_evaluator_bool():
    """Test boolean flag evaluation."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "boolFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    result = evaluator.evaluate_bool("boolFlag", {}, False)
    assert result is True


def test_flag_evaluator_string():
    """Test string flag evaluation."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "stringFlag": {
                "state": "ENABLED",
                "variants": {"red": "color-red", "blue": "color-blue"},
                "defaultVariant": "red"
            }
        }
    })

    result = evaluator.evaluate_string("stringFlag", {}, "default")
    assert result == "color-red"


def test_flag_evaluator_int():
    """Test integer flag evaluation."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "intFlag": {
                "state": "ENABLED",
                "variants": {"small": 10, "large": 100},
                "defaultVariant": "small"
            }
        }
    })

    result = evaluator.evaluate_int("intFlag", {}, 0)
    assert result == 10


def test_flag_evaluator_float():
    """Test float flag evaluation."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "floatFlag": {
                "state": "ENABLED",
                "variants": {"low": 1.5, "high": 9.9},
                "defaultVariant": "low"
            }
        }
    })

    result = evaluator.evaluate_float("floatFlag", {}, 0.0)
    assert result == 1.5


def test_flag_evaluator_no_state():
    """Test that evaluating without state returns the default value."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()

    # Should return default value when no state is loaded
    result = evaluator.evaluate_bool("myFlag", {}, False)
    assert result == False

    result2 = evaluator.evaluate_bool("myFlag", {}, True)
    assert result2 == True


def test_flag_evaluator_flag_not_found():
    """Test that evaluating non-existent flag returns the default value."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "existingFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    # Should return default value for non-existent flag
    result = evaluator.evaluate_bool("nonExistentFlag", {}, False)
    assert result == False

    result2 = evaluator.evaluate_string("nonExistentFlag", {}, "fallback")
    assert result2 == "fallback"
