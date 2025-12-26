"""Feature flag evaluation examples using FlagEvaluator."""

from flagd_evaluator import FlagEvaluator


def main():
    print("=== Feature Flag Evaluation Examples ===\n")

    # Create evaluator
    evaluator = FlagEvaluator()

    # Load configuration
    print("1. Loading flag configuration...")
    evaluator.update_state({
        "flags": {
            "darkMode": {
                "state": "ENABLED",
                "variants": {"on": True, "off": False},
                "defaultVariant": "off"
            },
            "theme": {
                "state": "ENABLED",
                "variants": {
                    "blue": {"color": "#0066cc", "name": "Ocean"},
                    "green": {"color": "#00cc66", "name": "Forest"},
                    "red": {"color": "#cc0000", "name": "Sunset"}
                },
                "defaultVariant": "blue"
            },
            "maxUploadSize": {
                "state": "ENABLED",
                "variants": {
                    "small": 5242880,      # 5MB
                    "medium": 52428800,    # 50MB
                    "large": 524288000     # 500MB
                },
                "defaultVariant": "small"
            },
            "discountRate": {
                "state": "ENABLED",
                "variants": {
                    "none": 0.0,
                    "small": 0.1,
                    "large": 0.25
                },
                "defaultVariant": "none"
            },
            "featureRollout": {
                "state": "ENABLED",
                "variants": {"enabled": True, "disabled": False},
                "defaultVariant": "disabled",
                "targeting": {
                    "fractional": [
                        {"var": "userId"},
                        ["enabled", 25],
                        ["disabled", 75]
                    ]
                }
            },
            "premiumFeatures": {
                "state": "ENABLED",
                "variants": {"enabled": True, "disabled": False},
                "defaultVariant": "disabled",
                "targeting": {
                    "if": [
                        {"==": [{"var": "tier"}, "premium"]},
                        "enabled",
                        "disabled"
                    ]
                }
            }
        }
    })
    print("   Configuration loaded!\n")

    # Boolean flag
    print("2. Boolean flag (darkMode):")
    dark_mode = evaluator.evaluate_bool("darkMode", {}, False)
    print(f"   Dark mode enabled: {dark_mode}\n")

    # Object flag
    print("3. Object flag (theme):")
    result = evaluator.evaluate("theme", {})
    print(f"   Theme: {result['value']}")
    print(f"   Variant: {result['variant']}")
    print(f"   Reason: {result['reason']}\n")

    # Integer flag
    print("4. Integer flag (maxUploadSize):")
    max_size = evaluator.evaluate_int("maxUploadSize", {}, 0)
    print(f"   Max upload size: {max_size} bytes ({max_size / 1024 / 1024:.1f}MB)\n")

    # Float flag
    print("5. Float flag (discountRate):")
    discount = evaluator.evaluate_float("discountRate", {}, 0.0)
    print(f"   Discount rate: {discount * 100}%\n")

    # Fractional targeting (A/B test)
    print("6. Fractional targeting (featureRollout):")
    for user_id in ["alice", "bob", "charlie", "dave"]:
        enabled = evaluator.evaluate_bool(
            "featureRollout",
            {"userId": user_id},
            False
        )
        print(f"   User '{user_id}': {'ENABLED' if enabled else 'DISABLED'}")
    print()

    # Conditional targeting
    print("7. Conditional targeting (premiumFeatures):")
    for tier in ["free", "premium", "enterprise"]:
        enabled = evaluator.evaluate_bool(
            "premiumFeatures",
            {"tier": tier},
            False
        )
        print(f"   Tier '{tier}': {'ENABLED' if enabled else 'DISABLED'}")
    print()

    # Full evaluation result
    print("8. Full evaluation result:")
    result = evaluator.evaluate("premiumFeatures", {"tier": "premium"})
    print(f"   Value: {result['value']}")
    print(f"   Variant: {result['variant']}")
    print(f"   Reason: {result['reason']}")
    if 'errorCode' in result and result['errorCode']:
        print(f"   Error: {result['errorCode']}")


if __name__ == "__main__":
    main()
