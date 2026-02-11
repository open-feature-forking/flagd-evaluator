//go:build comparison

package evaluator

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"testing"

	jsonlogic "github.com/diegoholiveira/jsonlogic/v3"
)

// Comparison benchmarks: WASM evaluator vs native Go JSON Logic (diegoholiveira/jsonlogic/v3)

func BenchmarkWASM_SimpleFlag(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleFlagConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("simple-flag", emptyCtx)
	}
}

func BenchmarkNative_SimpleFlag(b *testing.B) {
	// Simulate a static flag evaluation with JSON Logic
	rule := `true`
	data := `{}`
	var buf bytes.Buffer

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		buf.Reset()
		jsonlogic.Apply(strings.NewReader(rule), strings.NewReader(data), &buf)
	}
}

func BenchmarkWASM_TargetingMatch(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("targeting-flag", smallCtx)
	}
}

func BenchmarkNative_TargetingMatch(b *testing.B) {
	ruleJSON := `{"if": [{"==": [{"var": "tier"}, "premium"]}, "on", "off"]}`
	dataJSON := `{"tier": "premium", "targetingKey": "user-123", "role": "admin", "region": "us-east", "score": 85}`
	var buf bytes.Buffer

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		buf.Reset()
		jsonlogic.Apply(strings.NewReader(ruleJSON), strings.NewReader(dataJSON), &buf)
	}
}

func BenchmarkWASM_LargeContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)
	ctx := makeLargeCtx()
	// Add 900 more attributes for 1000+ total
	for i := 100; i < 1000; i++ {
		ctx[fmt.Sprintf("attr_%d", i)] = fmt.Sprintf("value_%d", i)
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("targeting-flag", ctx)
	}
}

func BenchmarkNative_LargeContext(b *testing.B) {
	ruleJSON := `{"if": [{"==": [{"var": "tier"}, "premium"]}, "on", "off"]}`
	ctx := makeLargeCtx()
	for i := 100; i < 1000; i++ {
		ctx[fmt.Sprintf("attr_%d", i)] = fmt.Sprintf("value_%d", i)
	}
	dataBytes, _ := json.Marshal(ctx)
	dataJSON := string(dataBytes)
	var buf bytes.Buffer

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		buf.Reset()
		jsonlogic.Apply(strings.NewReader(ruleJSON), strings.NewReader(dataJSON), &buf)
	}
}

// ====================================================================
// Throughput comparison: 1000 evaluations across N goroutines
// Shows how WASM (mutex-bound) vs native jsonlogic (lock-free) scale.
// ====================================================================

const compThroughputOps = 1000

var compTargetingRule = `{"if": [{"==": [{"var": "tier"}, "premium"]}, "on", "off"]}`

func makeCompDataJSON() string {
	data, _ := json.Marshal(smallCtx)
	return string(data)
}

// WASM targeting throughput
func BenchmarkThroughput_WASM_Targeting_1G(b *testing.B) {
	benchWASMThroughput(b, 1)
}

func BenchmarkThroughput_WASM_Targeting_4G(b *testing.B) {
	benchWASMThroughput(b, 4)
}

func BenchmarkThroughput_WASM_Targeting_16G(b *testing.B) {
	benchWASMThroughput(b, 16)
}

// Native jsonlogic targeting throughput (lock-free, should scale linearly)
func BenchmarkThroughput_Native_Targeting_1G(b *testing.B) {
	benchNativeThroughput(b, 1)
}

func BenchmarkThroughput_Native_Targeting_4G(b *testing.B) {
	benchNativeThroughput(b, 4)
}

func BenchmarkThroughput_Native_Targeting_16G(b *testing.B) {
	benchNativeThroughput(b, 16)
}

// Large context throughput — WASM context filtering vs native full parse
func BenchmarkThroughput_WASM_LargeCtx_1G(b *testing.B) {
	benchWASMLargeCtxThroughput(b, 1)
}

func BenchmarkThroughput_WASM_LargeCtx_4G(b *testing.B) {
	benchWASMLargeCtxThroughput(b, 4)
}

func BenchmarkThroughput_WASM_LargeCtx_16G(b *testing.B) {
	benchWASMLargeCtxThroughput(b, 16)
}

func BenchmarkThroughput_Native_LargeCtx_1G(b *testing.B) {
	benchNativeLargeCtxThroughput(b, 1)
}

func BenchmarkThroughput_Native_LargeCtx_4G(b *testing.B) {
	benchNativeLargeCtxThroughput(b, 4)
}

func BenchmarkThroughput_Native_LargeCtx_16G(b *testing.B) {
	benchNativeLargeCtxThroughput(b, 16)
}

func benchWASMThroughput(b *testing.B, goroutines int) {
	b.Helper()
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)
	opsPerG := compThroughputOps / goroutines

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var wg sync.WaitGroup
		wg.Add(goroutines)
		for g := 0; g < goroutines; g++ {
			go func() {
				defer wg.Done()
				for j := 0; j < opsPerG; j++ {
					e.EvaluateFlag("targeting-flag", smallCtx)
				}
			}()
		}
		wg.Wait()
	}
}

func benchNativeThroughput(b *testing.B, goroutines int) {
	b.Helper()
	dataJSON := makeCompDataJSON()
	opsPerG := compThroughputOps / goroutines

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var wg sync.WaitGroup
		wg.Add(goroutines)
		for g := 0; g < goroutines; g++ {
			go func() {
				defer wg.Done()
				var buf bytes.Buffer
				for j := 0; j < opsPerG; j++ {
					buf.Reset()
					jsonlogic.Apply(strings.NewReader(compTargetingRule), strings.NewReader(dataJSON), &buf)
				}
			}()
		}
		wg.Wait()
	}
}

func benchWASMLargeCtxThroughput(b *testing.B, goroutines int) {
	b.Helper()
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)
	ctx := makeLargeCtx()
	for i := 100; i < 1000; i++ {
		ctx[fmt.Sprintf("attr_%d", i)] = fmt.Sprintf("value_%d", i)
	}
	opsPerG := compThroughputOps / goroutines

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var wg sync.WaitGroup
		wg.Add(goroutines)
		for g := 0; g < goroutines; g++ {
			go func() {
				defer wg.Done()
				for j := 0; j < opsPerG; j++ {
					e.EvaluateFlag("targeting-flag", ctx)
				}
			}()
		}
		wg.Wait()
	}
}

func benchNativeLargeCtxThroughput(b *testing.B, goroutines int) {
	b.Helper()
	ctx := makeLargeCtx()
	for i := 100; i < 1000; i++ {
		ctx[fmt.Sprintf("attr_%d", i)] = fmt.Sprintf("value_%d", i)
	}
	dataBytes, _ := json.Marshal(ctx)
	dataJSON := string(dataBytes)
	opsPerG := compThroughputOps / goroutines

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var wg sync.WaitGroup
		wg.Add(goroutines)
		for g := 0; g < goroutines; g++ {
			go func() {
				defer wg.Done()
				var buf bytes.Buffer
				for j := 0; j < opsPerG; j++ {
					buf.Reset()
					jsonlogic.Apply(strings.NewReader(compTargetingRule), strings.NewReader(dataJSON), &buf)
				}
			}()
		}
		wg.Wait()
	}
}
