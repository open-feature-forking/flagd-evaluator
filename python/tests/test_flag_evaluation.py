"""Tests for feature flag evaluation in flagd_evaluator."""

import pytest


def test_flag_with_targeting():
    """Test flag evaluation with targeting rules."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
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
                        "user"
                    ]
                }
            }
        }
    })

    # Admin user should get admin variant
    result = evaluator.evaluate("targetedFlag", {"role": "admin"})
    assert result["value"] == "admin-view"
    assert result["variant"] == "admin"

    # Regular user should get user variant
    result2 = evaluator.evaluate("targetedFlag", {"role": "user"})
    assert result2["value"] == "user-view"
    assert result2["variant"] == "user"


def test_disabled_flag():
    """Test that disabled flags are handled correctly."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "disabledFlag": {
                "state": "DISABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    result = evaluator.evaluate("disabledFlag", {})
    assert result["reason"] == "DISABLED"


def test_flag_with_metadata():
    """Test flag evaluation with metadata."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "metadata": {
            "environment": "production",
            "version": "1.0"
        },
        "flags": {
            "metadataFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on",
                "metadata": {
                    "description": "Test flag with metadata"
                }
            }
        }
    })

    result = evaluator.evaluate("metadataFlag", {})
    # Check that metadata is present in the result
    assert "flagMetadata" in result or "flag_metadata" in result


def test_multiple_flags():
    """Test evaluating multiple different flags."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            },
            "flag2": {
                "state": "ENABLED",
                "variants": {"red": "color-red", "blue": "color-blue"},
                "defaultVariant": "blue"
            },
            "flag3": {
                "state": "ENABLED",
                "variants": {"small": 10, "large": 100},
                "defaultVariant": "large"
            }
        }
    })

    result1 = evaluator.evaluate_bool("flag1", {}, False)
    assert result1 is True

    result2 = evaluator.evaluate_string("flag2", {}, "default")
    assert result2 == "color-blue"

    result3 = evaluator.evaluate_int("flag3", {}, 0)
    assert result3 == 100


def test_evaluate_full_result():
    """Test that evaluate method returns full result with all fields."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    result = evaluator.evaluate("testFlag", {})

    # Check that all expected fields are present
    assert "value" in result
    assert "variant" in result
    assert "reason" in result

    # Check values
    assert result["value"] is True
    assert result["variant"] == "on"
    assert result["reason"] in ["STATIC", "TARGETING_MATCH", "DEFAULT"]


def test_flag_with_fractional_targeting():
    """Test flag evaluation with fractional operator in targeting."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "abTestFlag": {
                "state": "ENABLED",
                "variants": {
                    "control": {"color": "blue", "size": "medium"},
                    "treatment": {"color": "green", "size": "large"}
                },
                "defaultVariant": "control",
                "targeting": {
                    "fractional": [
                        {"var": "userId"},
                        ["control", 50],
                        ["treatment", 50]
                    ]
                }
            }
        }
    })

    result = evaluator.evaluate("abTestFlag", {"userId": "user123"})
    assert result["variant"] in ["control", "treatment"]
    assert result["value"] in [
        {"color": "blue", "size": "medium"},
        {"color": "green", "size": "large"}
    ]


def test_complex_targeting_rule():
    """Test flag with complex targeting rule combining multiple operators."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "complexFlag": {
                "state": "ENABLED",
                "variants": {"premium": "premium-feature", "basic": "basic-feature"},
                "defaultVariant": "basic",
                "targeting": {
                    "if": [
                        {
                            "and": [
                                {">": [{"var": "age"}, 18]},
                                {"starts_with": [{"var": "email"}, "premium@"]}
                            ]
                        },
                        "premium",
                        "basic"
                    ]
                }
            }
        }
    })

    # Premium user
    result = evaluator.evaluate("complexFlag", {
        "age": 25,
        "email": "premium@example.com"
    })
    assert result["value"] == "premium-feature"

    # Basic user (wrong email)
    result2 = evaluator.evaluate("complexFlag", {
        "age": 25,
        "email": "user@example.com"
    })
    assert result2["value"] == "basic-feature"

    # Basic user (too young)
    result3 = evaluator.evaluate("complexFlag", {
        "age": 16,
        "email": "premium@example.com"
    })
    assert result3["value"] == "basic-feature"
