"""Examples demonstrating custom operators in flagd-evaluator."""

from flagd_evaluator import evaluate_logic


def main():
    print("=== Custom Operators Examples ===\n")

    # Fractional operator for A/B testing
    print("1. FRACTIONAL operator (A/B testing):")
    print("   Consistently buckets users into variants based on hash\n")

    users = ["alice", "bob", "charlie", "dave", "eve"]
    buckets = {}

    for user in users:
        result = evaluate_logic(
            {"fractional": [
                {"var": "userId"},
                ["control", 50],
                ["treatment", 50]
            ]},
            {"userId": user}
        )
        bucket = result["result"]
        buckets[user] = bucket
        print(f"   User '{user}' → {bucket}")

    print(f"\n   Distribution: {list(buckets.values()).count('control')} control, "
          f"{list(buckets.values()).count('treatment')} treatment\n")

    # Semantic version comparison
    print("2. SEM_VER operator (version comparison):")

    version_tests = [
        ("=", "1.0.0", "1.0.0", "exact match"),
        ("!=", "1.0.0", "2.0.0", "not equal"),
        (">", "2.0.0", "1.0.0", "greater than"),
        (">=", "2.0.0", "2.0.0", "greater or equal"),
        ("<", "1.0.0", "2.0.0", "less than"),
        ("<=", "1.5.0", "1.5.0", "less or equal"),
        ("^", "1.5.0", "1.0.0", "caret range (^1.0.0)"),
        ("~", "1.0.5", "1.0.0", "tilde range (~1.0.0)"),
    ]

    for op, v1, v2, desc in version_tests:
        result = evaluate_logic(
            {"sem_ver": [op, v1, v2]},
            {}
        )
        symbol = "✓" if result["result"] else "✗"
        print(f"   {symbol} {v1} {op} {v2} ({desc})")

    print()

    # String operators
    print("3. STRING operators (starts_with, ends_with):")

    email_tests = [
        ("admin@example.com", "starts_with", "admin@", True),
        ("user@example.com", "starts_with", "admin@", False),
        ("test@example.com", "ends_with", "@example.com", True),
        ("test@other.org", "ends_with", "@example.com", False),
    ]

    for email, op, pattern, expected in email_tests:
        result = evaluate_logic(
            {op: [{"var": "email"}, pattern]},
            {"email": email}
        )
        match = "matches" if result["result"] else "doesn't match"
        symbol = "✓" if result["result"] == expected else "✗"
        print(f"   {symbol} '{email}' {match} '{pattern}'")

    print()

    # Combined operators in targeting
    print("4. COMBINED operators (complex targeting):")

    rule = {
        "and": [
            # User has premium email
            {"starts_with": [{"var": "email"}, "premium@"]},
            # App version is at least 2.0.0
            {"sem_ver": [">=", {"var": "appVersion"}, "2.0.0"]},
            # User is in treatment bucket
            {"==": [
                {"fractional": [
                    {"var": "userId"},
                    ["control", 50],
                    ["treatment", 50]
                ]},
                "treatment"
            ]}
        ]
    }

    test_users = [
        {
            "email": "premium@example.com",
            "appVersion": "2.1.0",
            "userId": "alice"
        },
        {
            "email": "user@example.com",
            "appVersion": "2.1.0",
            "userId": "bob"
        },
        {
            "email": "premium@example.com",
            "appVersion": "1.5.0",
            "userId": "charlie"
        },
    ]

    for user in test_users:
        result = evaluate_logic(rule, user)
        status = "ELIGIBLE" if result["result"] else "NOT ELIGIBLE"
        print(f"   {user['email']}, v{user['appVersion']}, {user['userId']}: {status}")


if __name__ == "__main__":
    main()
