# flagd-evaluator Go Package

Pure Go package for evaluating feature flags using the flagd-evaluator WASM module. Uses [wazero](https://wazero.io/) for zero-CGO WebAssembly execution.

## Features

- **Pure Go** — no CGO, no native dependencies
- **Embedded WASM** — binary bundled via `//go:embed` (~2.7MB)
- **Thread-safe** — safe for concurrent use from multiple goroutines
- **3 host-side optimizations**:
  - Pre-evaluation cache for static/disabled flags
  - Context key filtering (only serialize keys referenced by targeting rules)
  - Index-based WASM evaluation (O(1) flag lookup, no string serialization)

## Installation

```bash
go get github.com/open-feature/flagd-evaluator/go
```

## Usage

```go
package main

import (
	"fmt"
	"log"

	evaluator "github.com/open-feature/flagd-evaluator/go"
)

func main() {
	// Create evaluator (compiles WASM module)
	e, err := evaluator.NewFlagEvaluator(evaluator.WithPermissiveValidation())
	if err != nil {
		log.Fatal(err)
	}
	defer e.Close()

	// Load flag configuration
	config := `{
		"flags": {
			"my-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [
						{ "==": [{ "var": "email" }, "admin@example.com"] },
						"on", "off"
					]
				}
			}
		}
	}`
	result, err := e.UpdateState(config)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("Changed flags:", result.ChangedFlags)

	// Evaluate with context
	ctx := map[string]interface{}{
		"targetingKey": "user-123",
		"email":        "admin@example.com",
	}
	evalResult, err := e.EvaluateFlag("my-flag", ctx)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("Value: %v, Variant: %s, Reason: %s\n",
		evalResult.Value, evalResult.Variant, evalResult.Reason)

	// Typed convenience methods (return default on error)
	val := e.EvaluateBool("my-flag", ctx, false)
	fmt.Println("Bool value:", val)
}
```

## API

### Lifecycle

```go
func NewFlagEvaluator(opts ...Option) (*FlagEvaluator, error)
func (e *FlagEvaluator) Close() error
```

### Options

```go
func WithPermissiveValidation() Option  // Accept invalid configs with warnings
```

### State Management

```go
func (e *FlagEvaluator) UpdateState(configJSON string) (*UpdateStateResult, error)
```

### Evaluation

```go
// Full result
func (e *FlagEvaluator) EvaluateFlag(flagKey string, ctx map[string]interface{}) (*EvaluationResult, error)

// Typed (return default on error)
func (e *FlagEvaluator) EvaluateBool(flagKey string, ctx map[string]interface{}, defaultValue bool) bool
func (e *FlagEvaluator) EvaluateString(flagKey string, ctx map[string]interface{}, defaultValue string) string
func (e *FlagEvaluator) EvaluateInt(flagKey string, ctx map[string]interface{}, defaultValue int64) int64
func (e *FlagEvaluator) EvaluateFloat(flagKey string, ctx map[string]interface{}, defaultValue float64) float64
```

## Building

```bash
# Build WASM from Rust source (requires Rust toolchain)
make wasm

# Run tests
make test

# Run benchmarks
make bench

# Run comparison benchmarks (vs diegoholiveira/jsonlogic/v3)
make bench-comparison
```

## Performance

The host-side optimizations provide significant speedups:

| Scenario | Description |
|----------|-------------|
| Static/disabled flags | ~0 ns (pre-evaluated cache, no WASM call) |
| Targeting (small context) | Filtered context + index-based evaluation |
| Targeting (large context) | Only needed keys serialized, massive savings |

Run `make bench` to see results on your hardware.
