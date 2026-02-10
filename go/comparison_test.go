//go:build comparison

package evaluator

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"
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
