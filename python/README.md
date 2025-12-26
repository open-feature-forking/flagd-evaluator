# flagd-evaluator Python Bindings

Native Python bindings for the [flagd-evaluator](https://github.com/open-feature-forking/flagd-evaluator) library, providing high-performance feature flag evaluation with JSON Logic support.

## Features

- **Native Performance**: Direct Rust-to-Python compilation using PyO3
- **Pythonic API**: Natural Python dictionaries and types
- **Full JSON Logic Support**: All standard operators plus custom operators
- **Custom Operators**: `fractional` (A/B testing), `sem_ver`, `starts_with`, `ends_with`
- **Type Hints**: Complete type stubs for IDE support
- **Zero Configuration**: No WASM runtime required

## Installation

```bash
pip install flagd-evaluator
```

## Quick Start

### Stateful Flag Evaluation

```python
from flagd_evaluator import FlagEvaluator

# Create evaluator
evaluator = FlagEvaluator()

# Load flag configuration
evaluator.update_state({
    "flags": {
        "myFlag": {
            "state": "ENABLED",
            "variants": {"on": True, "off": False},
            "defaultVariant": "on"
        }
    }
})

# Evaluate flag
result = evaluator.evaluate_bool("myFlag", {}, False)
print(result)  # True
```

## API Reference

### FlagEvaluator

Stateful feature flag evaluator class.

#### Methods

##### `__init__()`
Create a new FlagEvaluator instance.

##### `update_state(config: dict) -> dict`
Update the flag configuration state.

**Parameters:**
- `config` (dict): Flag configuration in flagd format

**Returns:**
- dict with `success` status

##### `evaluate(flag_key: str, context: dict) -> dict`
Evaluate a feature flag and return full result.

**Parameters:**
- `flag_key` (str): The flag key to evaluate
- `context` (dict): Evaluation context

**Returns:**
- dict with keys: `value`, `variant`, `reason`, `flagMetadata`

##### `evaluate_bool(flag_key: str, context: dict, default_value: bool) -> bool`
Evaluate a boolean flag.

##### `evaluate_string(flag_key: str, context: dict, default_value: str) -> str`
Evaluate a string flag.

##### `evaluate_int(flag_key: str, context: dict, default_value: int) -> int`
Evaluate an integer flag.

##### `evaluate_float(flag_key: str, context: dict, default_value: float) -> float`
Evaluate a float flag.

## Custom Operators

### fractional - A/B Testing

Consistently bucket users into variants based on a hash:

```python
from flagd_evaluator import evaluate_logic

result = evaluate_logic(
    {"fractional": [{"var": "userId"}, ["A", 50], ["B", 50]]},
    {"userId": "user123"}
)
print(result["result"])  # "A" or "B" (consistent for same userId)
```

### sem_ver - Semantic Version Comparison

Compare semantic versions:

```python
result = evaluate_logic(
    {"sem_ver": [">=", "2.1.0", "2.0.0"]},
    {}
)
print(result["result"])  # True

# Caret range (compatible versions)
result = evaluate_logic(
    {"sem_ver": ["^", "1.5.0", "1.0.0"]},
    {}
)
print(result["result"])  # True (1.5.0 matches ^1.0.0)
```

### String Operators

```python
# starts_with
result = evaluate_logic(
    {"starts_with": [{"var": "email"}, "admin@"]},
    {"email": "admin@example.com"}
)
print(result["result"])  # True

# ends_with
result = evaluate_logic(
    {"ends_with": [{"var": "domain"}, ".com"]},
    {"domain": "example.com"}
)
print(result["result"])  # True
```

## Examples

See the [examples/](examples/) directory for more examples:
- `flag_evaluation.py` - Stateful flag evaluation with various scenarios

## Performance

Native Python bindings offer significant performance improvements over WASM-based approaches:

- **5-10x faster** evaluation
- **No WASM overhead**
- **Direct memory sharing** between Rust and Python
- **Native Python exceptions** with full stack traces

See [benchmarks/bench_vs_wasm.py](benchmarks/bench_vs_wasm.py) for detailed comparisons.

## Development

### Building from Source

```bash
# Install maturin
pip install maturin

# Build and install locally
cd python
maturin develop

# Run tests
pytest tests/ -v
```

### Running Tests

```bash
pip install pytest
pytest python/tests/ -v
```

## Comparison: Native vs WASM

| Feature | Native (PyO3) | WASM |
|---------|---------------|------|
| Performance | ⚡ 5-10x faster | Slower |
| Installation | `pip install` | Manual setup |
| API | Pythonic dicts | JSON strings |
| Memory | Shared | Separate |
| Error handling | Python exceptions | JSON errors |
| Dependencies | None | `wasmtime-py` |

## License

Apache-2.0

## Contributing

Contributions are welcome! Please see the main repository's [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## Related

- [Main flagd-evaluator repository](https://github.com/open-feature-forking/flagd-evaluator)
- [PyO3 Documentation](https://pyo3.rs)
- [JSON Logic](https://jsonlogic.com/)
