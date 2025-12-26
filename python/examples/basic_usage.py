"""Basic usage examples for flagd-evaluator Python bindings."""

from flagd_evaluator import evaluate_logic


def main():
    print("=== Basic JSON Logic Evaluation ===\n")

    # Simple equality
    print("1. Simple equality:")
    result = evaluate_logic({"==": [1, 1]}, {})
    print(f"   {{'==': [1, 1]}} => {result['result']}")
    print(f"   Full result: {result}\n")

    # Variable lookup
    print("2. Variable lookup:")
    result = evaluate_logic(
        {">": [{"var": "age"}, 18]},
        {"age": 25}
    )
    print(f"   age > 18 (age=25) => {result['result']}\n")

    # Nested conditions
    print("3. Nested conditions:")
    result = evaluate_logic(
        {
            "and": [
                {">": [{"var": "age"}, 18]},
                {"<": [{"var": "age"}, 65]}
            ]
        },
        {"age": 30}
    )
    print(f"   18 < age < 65 (age=30) => {result['result']}\n")

    # Array operations
    print("4. Array operations:")
    result = evaluate_logic(
        {"in": [{"var": "role"}, ["admin", "moderator", "editor"]]},
        {"role": "admin"}
    )
    print(f"   role in ['admin', 'moderator', 'editor'] => {result['result']}\n")

    # Missing operation
    print("5. Missing values:")
    result = evaluate_logic(
        {"missing": ["email", "age"]},
        {"email": "user@example.com"}
    )
    print(f"   Missing fields: {result['result']}\n")

    # Map operation
    print("6. Map operation:")
    result = evaluate_logic(
        {"map": [
            {"var": "users"},
            {"var": "name"}
        ]},
        {"users": [{"name": "Alice"}, {"name": "Bob"}, {"name": "Charlie"}]}
    )
    print(f"   Extract names: {result['result']}\n")

    # Filter operation
    print("7. Filter operation:")
    result = evaluate_logic(
        {"filter": [
            {"var": "users"},
            {">": [{"var": "age"}, 21]}
        ]},
        {
            "users": [
                {"name": "Alice", "age": 25},
                {"name": "Bob", "age": 18},
                {"name": "Charlie", "age": 30}
            ]
        }
    )
    print(f"   Users over 21: {result['result']}\n")


if __name__ == "__main__":
    main()
