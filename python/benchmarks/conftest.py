"""Shared fixtures for flagd-evaluator benchmarks."""

import pytest
from flagd_evaluator import FlagEvaluator


def _build_flag_config():
    """Build a rich flag configuration for benchmarks."""
    return {
        "flags": {
            # Simple boolean flag (no targeting)
            "simple-bool": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on",
            },
            # Boolean flag with targeting rule that matches
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
            # String flag
            "string-flag": {
                "state": "ENABLED",
                "variants": {
                    "banner-a": "Welcome to our new experience!",
                    "banner-b": "Check out our latest features!",
                    "default-banner": "Welcome!",
                },
                "defaultVariant": "default-banner",
                "targeting": {
                    "if": [
                        {"==": [{"var": "segment"}, "beta"]},
                        "banner-a",
                        {"if": [
                            {"==": [{"var": "segment"}, "internal"]},
                            "banner-b",
                            "default-banner",
                        ]},
                    ]
                },
            },
            # Integer flag
            "int-flag": {
                "state": "ENABLED",
                "variants": {"low": 10, "medium": 50, "high": 100},
                "defaultVariant": "medium",
            },
            # Float flag
            "float-flag": {
                "state": "ENABLED",
                "variants": {"conservative": 0.1, "moderate": 0.5, "aggressive": 0.9},
                "defaultVariant": "moderate",
            },
            # Object flag
            "object-flag": {
                "state": "ENABLED",
                "variants": {
                    "config-a": {
                        "color": "blue",
                        "size": "large",
                        "features": ["search", "export"],
                    },
                    "config-b": {
                        "color": "green",
                        "size": "medium",
                        "features": ["search"],
                    },
                },
                "defaultVariant": "config-a",
            },
            # Disabled flag
            "disabled-flag": {
                "state": "DISABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "on",
            },
            # Fractional bucketing flag
            "fractional-flag": {
                "state": "ENABLED",
                "variants": {
                    "control": "control-experience",
                    "treatment-a": "treatment-a-experience",
                    "treatment-b": "treatment-b-experience",
                },
                "defaultVariant": "control",
                "targeting": {
                    "fractional": [
                        {"var": "targetingKey"},
                        ["control", 50],
                        ["treatment-a", 25],
                        ["treatment-b", 25],
                    ]
                },
            },
            # Fractional bucketing flag with 8 weighted buckets (O2)
            "fractional-8-flag": {
                "state": "ENABLED",
                "variants": {
                    "v1": "v1",
                    "v2": "v2",
                    "v3": "v3",
                    "v4": "v4",
                    "v5": "v5",
                    "v6": "v6",
                    "v7": "v7",
                    "v8": "v8",
                },
                "defaultVariant": "v1",
                "targeting": {
                    "fractional": [
                        {"var": "targetingKey"},
                        ["v1", 12],
                        ["v2", 13],
                        ["v3", 12],
                        ["v4", 13],
                        ["v5", 12],
                        ["v6", 13],
                        ["v7", 12],
                        ["v8", 13],
                    ]
                },
            },
            # Semver flag (equality, O3)
            "semver-flag": {
                "state": "ENABLED",
                "variants": {"new-ui": True, "old-ui": False},
                "defaultVariant": "old-ui",
                "targeting": {
                    "if": [
                        {"sem_ver": [{"var": "appVersion"}, ">=", "2.0.0"]},
                        "new-ui",
                        "old-ui",
                    ]
                },
            },
            # Semver range flag with caret operator (O4)
            "semver-range-flag": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "off",
                "targeting": {
                    "if": [
                        {"sem_ver": [{"var": "version"}, "^", "1.2.0"]},
                        "on",
                        "off",
                    ]
                },
            },
            # starts_with flag
            "starts-with-flag": {
                "state": "ENABLED",
                "variants": {"internal": "internal-access", "external": "external-access"},
                "defaultVariant": "external",
                "targeting": {
                    "if": [
                        {"starts_with": [{"var": "email"}, "admin@"]},
                        "internal",
                        "external",
                    ]
                },
            },
            # ends_with flag
            "ends-with-flag": {
                "state": "ENABLED",
                "variants": {"corp": "corporate-plan", "personal": "personal-plan"},
                "defaultVariant": "personal",
                "targeting": {
                    "if": [
                        {"ends_with": [{"var": "email"}, "@corp.example.com"]},
                        "corp",
                        "personal",
                    ]
                },
            },
            # Complex targeting flag (nested if/and/or)
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
                        {"and": [
                            {"==": [{"var": "tier"}, "premium"]},
                            {">": [{"var": "score"}, 90]},
                        ]},
                        "premium",
                        {"if": [
                            {"or": [
                                {"==": [{"var": "tier"}, "standard"]},
                                {">": [{"var": "score"}, 50]},
                            ]},
                            "standard",
                            "basic",
                        ]},
                    ]
                },
            },
        }
    }


@pytest.fixture
def flag_config():
    """Raw flag configuration dict for state-update benchmarks."""
    return _build_flag_config()


@pytest.fixture
def evaluator():
    """FlagEvaluator preloaded with a rich flag configuration."""
    ev = FlagEvaluator()
    ev.update_state(_build_flag_config())
    return ev


@pytest.fixture
def small_context():
    """Evaluation context with 5 attributes."""
    return {
        "targetingKey": "user-123",
        "tier": "premium",
        "role": "admin",
        "region": "us-east",
        "score": 85,
    }


@pytest.fixture
def large_context():
    """Evaluation context with 100+ attributes."""
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
    # Add 100 additional attributes
    for i in range(100):
        ctx[f"attr_{i}"] = f"value_{i}"
    return ctx
