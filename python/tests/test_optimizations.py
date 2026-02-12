"""Tests for host-side optimization features: pre-evaluation cache,
context key filtering, and index-based evaluation."""

import pytest
import time


def test_update_state_returns_pre_evaluated():
    """Static/disabled flags should appear in preEvaluated response."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    result = evaluator.update_state({
        "flags": {
            "staticFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            },
            "disabledFlag": {
                "state": "DISABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            },
            "targetedFlag": {
                "state": "ENABLED",
                "variants": {"a": "val-a", "b": "val-b"},
                "defaultVariant": "a",
                "targeting": {
                    "if": [
                        {"==": [{"var": "role"}, "admin"]},
                        "b",
                        "a"
                    ]
                }
            }
        }
    })

    assert result["success"] is True
    pre_evaluated = result.get("preEvaluated", {})
    assert pre_evaluated is not None

    # Static flag should be pre-evaluated
    assert "staticFlag" in pre_evaluated
    assert pre_evaluated["staticFlag"]["value"] is True
    assert pre_evaluated["staticFlag"]["reason"] == "STATIC"

    # Disabled flag should be pre-evaluated
    assert "disabledFlag" in pre_evaluated
    assert pre_evaluated["disabledFlag"]["reason"] == "DISABLED"

    # Targeted flag should NOT be pre-evaluated
    assert "targetedFlag" not in pre_evaluated


def test_static_flag_served_from_cache():
    """Evaluating a static flag should return the cached pre-evaluated result."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "cachedFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    # Evaluate multiple times; result should always be the cached value
    for _ in range(10):
        result = evaluator.evaluate("cachedFlag", {})
        assert result["value"] is True
        assert result["variant"] == "on"
        assert result["reason"] == "STATIC"


def test_disabled_flag_served_from_cache():
    """Evaluating a disabled flag should return the cached pre-evaluated result."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "offFlag": {
                "state": "DISABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    result = evaluator.evaluate("offFlag", {})
    assert result["reason"] == "DISABLED"


def test_disabled_flag_bool_returns_default():
    """Disabled flags should return the caller-supplied default via evaluate_bool."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "offFlag": {
                "state": "DISABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    assert evaluator.evaluate_bool("offFlag", {}, True) is True
    assert evaluator.evaluate_bool("offFlag", {}, False) is False


def test_update_state_returns_required_context_keys():
    """Targeted flags should have requiredContextKeys in the response."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    result = evaluator.update_state({
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

    assert result["success"] is True
    req_keys = result.get("requiredContextKeys", {})
    assert req_keys is not None
    assert "targetedFlag" in req_keys
    keys_list = req_keys["targetedFlag"]
    assert "role" in keys_list
    assert "targetingKey" in keys_list


def test_update_state_returns_flag_indices():
    """All flags should have numeric indices in the response."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    result = evaluator.update_state({
        "flags": {
            "flagB": {
                "state": "ENABLED",
                "variants": {"on": True},
                "defaultVariant": "on"
            },
            "flagA": {
                "state": "ENABLED",
                "variants": {"off": False},
                "defaultVariant": "off"
            }
        }
    })

    assert result["success"] is True
    indices = result.get("flagIndices", {})
    assert indices is not None
    # Indices are assigned in sorted order
    assert indices["flagA"] == 0
    assert indices["flagB"] == 1


def test_filtered_context_targeting_produces_correct_result():
    """Targeted flags with required keys should evaluate correctly
    even when extra context keys are present."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "roleFlag": {
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

    # Pass many extra keys that the targeting rule does NOT reference
    context = {
        "role": "admin",
        "targetingKey": "user-123",
        "email": "admin@example.com",
        "age": 30,
        "plan": "enterprise",
        "country": "US",
        "extra1": "val1",
        "extra2": "val2",
    }

    result = evaluator.evaluate("roleFlag", context)
    assert result["value"] == "admin-view"
    assert result["variant"] == "admin"
    assert result["reason"] == "TARGETING_MATCH"


def test_filtered_context_user_variant():
    """Non-matching targeted flag should return the default variant."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "roleFlag": {
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

    result = evaluator.evaluate("roleFlag", {"role": "user"})
    assert result["value"] == "user-view"
    assert result["variant"] == "user"


def test_filtered_context_with_multiple_required_keys():
    """Flags referencing multiple context keys should filter correctly."""
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

    # Premium user with extra keys
    result = evaluator.evaluate("complexFlag", {
        "age": 25,
        "email": "premium@example.com",
        "extra1": "ignored",
        "extra2": "also-ignored",
    })
    assert result["value"] == "premium-feature"

    # Non-premium user
    result2 = evaluator.evaluate("complexFlag", {
        "age": 25,
        "email": "user@example.com",
    })
    assert result2["value"] == "basic-feature"


def test_flagd_enrichment_in_filtered_context():
    """Targeting rules referencing $flagd properties should work with filtered context."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "enrichedFlag": {
                "state": "ENABLED",
                "variants": {"yes": "flagd-works", "no": "flagd-missing"},
                "defaultVariant": "no",
                "targeting": {
                    "if": [
                        {
                            "and": [
                                {"==": [{"var": "$flagd.flagKey"}, "enrichedFlag"]},
                                {">": [{"var": "$flagd.timestamp"}, 0]}
                            ]
                        },
                        "yes",
                        "no"
                    ]
                }
            }
        }
    })

    result = evaluator.evaluate("enrichedFlag", {})
    assert result["value"] == "flagd-works"
    assert result["variant"] == "yes"


def test_fractional_targeting_with_filtered_context():
    """Fractional operator should work correctly with filtered context
    (targetingKey is always included)."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "abFlag": {
                "state": "ENABLED",
                "variants": {
                    "control": "control-value",
                    "treatment": "treatment-value"
                },
                "defaultVariant": "control",
                "targeting": {
                    "fractional": [
                        ["control", 50],
                        ["treatment", 50]
                    ]
                }
            }
        }
    })

    # targetingKey should be automatically included in filtered context
    result = evaluator.evaluate("abFlag", {"targetingKey": "user-abc"})
    assert result["variant"] in ["control", "treatment"]
    assert result["value"] in ["control-value", "treatment-value"]
    assert result["reason"] == "TARGETING_MATCH"


def test_cache_refreshed_on_update_state():
    """Calling update_state again should refresh all caches."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()

    # Initial state: flag is static with value True
    evaluator.update_state({
        "flags": {
            "dynamicFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on"
            }
        }
    })

    result1 = evaluator.evaluate("dynamicFlag", {})
    assert result1["value"] is True
    assert result1["reason"] == "STATIC"

    # Update: now flag has targeting
    evaluator.update_state({
        "flags": {
            "dynamicFlag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "off",
                "targeting": {
                    "if": [
                        {"==": [{"var": "enabled"}, True]},
                        "on",
                        "off"
                    ]
                }
            }
        }
    })

    result2 = evaluator.evaluate("dynamicFlag", {"enabled": True})
    assert result2["value"] is True
    assert result2["reason"] == "TARGETING_MATCH"

    result3 = evaluator.evaluate("dynamicFlag", {"enabled": False})
    assert result3["value"] is False


def test_static_flags_not_in_required_context_keys():
    """Static flags should not appear in requiredContextKeys."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    result = evaluator.update_state({
        "flags": {
            "staticFlag": {
                "state": "ENABLED",
                "variants": {"on": True},
                "defaultVariant": "on"
            },
            "targetedFlag": {
                "state": "ENABLED",
                "variants": {"a": "val-a", "b": "val-b"},
                "defaultVariant": "a",
                "targeting": {
                    "if": [{"==": [{"var": "tier"}, "premium"]}, "b", "a"]
                }
            }
        }
    })

    req_keys = result.get("requiredContextKeys", {})
    # Static flags should not appear in required context keys
    assert "staticFlag" not in req_keys
    # Targeted flags should
    assert "targetedFlag" in req_keys
    assert "tier" in req_keys["targetedFlag"]


def test_evaluate_typed_with_filtered_context():
    """Typed evaluation methods should also use optimized paths."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "boolTarget": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "off",
                "targeting": {
                    "if": [{"==": [{"var": "role"}, "admin"]}, "on", "off"]
                }
            },
            "stringTarget": {
                "state": "ENABLED",
                "variants": {"greeting": "Hello!", "farewell": "Goodbye!"},
                "defaultVariant": "farewell",
                "targeting": {
                    "if": [{"==": [{"var": "mood"}, "happy"]}, "greeting", "farewell"]
                }
            },
            "intTarget": {
                "state": "ENABLED",
                "variants": {"low": 10, "high": 100},
                "defaultVariant": "low",
                "targeting": {
                    "if": [{">": [{"var": "level"}, 5]}, "high", "low"]
                }
            }
        }
    })

    assert evaluator.evaluate_bool("boolTarget", {"role": "admin"}, False) is True
    assert evaluator.evaluate_bool("boolTarget", {"role": "user"}, False) is False
    assert evaluator.evaluate_string("stringTarget", {"mood": "happy"}, "x") == "Hello!"
    assert evaluator.evaluate_int("intTarget", {"level": 10}, 0) == 100
    assert evaluator.evaluate_int("intTarget", {"level": 1}, 0) == 10


def test_targeting_key_defaults_to_empty_in_filtered_context():
    """When targetingKey is not in the context, it should default to empty string."""
    from flagd_evaluator import FlagEvaluator

    evaluator = FlagEvaluator()
    evaluator.update_state({
        "flags": {
            "tkFlag": {
                "state": "ENABLED",
                "variants": {"present": "has-tk", "absent": "no-tk"},
                "defaultVariant": "absent",
                "targeting": {
                    "if": [
                        {"!=": [{"var": "targetingKey"}, ""]},
                        "present",
                        "absent"
                    ]
                }
            }
        }
    })

    # No targetingKey provided -> should be empty string -> "absent"
    result_no_tk = evaluator.evaluate("tkFlag", {})
    assert result_no_tk["value"] == "no-tk"

    # With targetingKey -> "present"
    result_with_tk = evaluator.evaluate("tkFlag", {"targetingKey": "user-1"})
    assert result_with_tk["value"] == "has-tk"
