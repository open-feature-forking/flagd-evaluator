import json
import pytest
from flagd_evaluator import FlagEvaluator

SIMPLE_YAML = """
flags:
  bool-flag:
    state: ENABLED
    variants:
      "on": true
      "off": false
    defaultVariant: "on"
  string-flag:
    state: ENABLED
    variants:
      v1: hello
      v2: world
    defaultVariant: v1
  int-flag:
    state: ENABLED
    variants:
      low: 10
      high: 100
    defaultVariant: low
"""

YAML_WITH_TARGETING = """
flags:
  targeted-flag:
    state: ENABLED
    variants:
      "yes": true
      "no": false
    defaultVariant: "no"
    targeting:
      if:
        - ==:
            - var: targetingKey
            - admin
        - "yes"
        - "no"
"""


def test_update_state_yaml_loads_bool_flag():
    evaluator = FlagEvaluator()
    evaluator.update_state(SIMPLE_YAML)
    result = evaluator.evaluate_bool("bool-flag", {}, False)
    assert result is True


def test_update_state_yaml_loads_string_flag():
    evaluator = FlagEvaluator()
    evaluator.update_state(SIMPLE_YAML)
    result = evaluator.evaluate_string("string-flag", {}, "default")
    assert result == "hello"


def test_update_state_yaml_loads_int_flag():
    evaluator = FlagEvaluator()
    evaluator.update_state(SIMPLE_YAML)
    result = evaluator.evaluate_int("int-flag", {}, 0)
    assert result == 10


def test_update_state_yaml_invalid_yaml_raises():
    evaluator = FlagEvaluator()
    with pytest.raises(ValueError, match="Failed to parse YAML"):
        evaluator.update_state("flags:\n  bad: [unclosed")


def test_update_state_yaml_with_targeting():
    evaluator = FlagEvaluator()
    evaluator.update_state(YAML_WITH_TARGETING)
    assert evaluator.evaluate_bool("targeted-flag", {"targetingKey": "admin"}, False) is True
    assert evaluator.evaluate_bool("targeted-flag", {"targetingKey": "user"}, True) is False


def test_yaml_and_json_produce_same_results():
    yaml_evaluator = FlagEvaluator()
    yaml_evaluator.update_state(SIMPLE_YAML)

    json_evaluator = FlagEvaluator()
    json_evaluator.update_state(json.dumps({"flags": {
        "bool-flag": {"state": "ENABLED", "variants": {"on": True, "off": False}, "defaultVariant": "on"},
        "string-flag": {"state": "ENABLED", "variants": {"v1": "hello", "v2": "world"}, "defaultVariant": "v1"},
        "int-flag": {"state": "ENABLED", "variants": {"low": 10, "high": 100}, "defaultVariant": "low"}
    }}))

    yaml_result = yaml_evaluator.evaluate_bool("bool-flag", {}, False)
    json_result = json_evaluator.evaluate_bool("bool-flag", {}, False)
    assert yaml_result == json_result


def test_update_state_yaml_missing_flags_key_raises_strict():
    evaluator = FlagEvaluator()  # strict mode by default
    # update_state returns a response dict with success=False for schema failures (doesn't raise)
    result = evaluator.update_state("foo: bar\n")
    assert result["success"] is False
    assert result["error"] is not None
