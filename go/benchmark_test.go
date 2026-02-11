package evaluator

import (
	"fmt"
	"sync"
	"testing"
)

// Standard context definitions from BENCHMARKS.md

var emptyCtx = map[string]interface{}{}

var smallCtx = map[string]interface{}{
	"targetingKey": "user-123",
	"tier":         "premium",
	"role":         "admin",
	"region":       "us-east",
	"score":        85,
}

func makeLargeCtx() map[string]interface{} {
	ctx := map[string]interface{}{
		"targetingKey": "user-123",
		"tier":         "premium",
		"role":         "admin",
		"region":       "us-east",
		"score":        85,
	}
	for i := 0; i < 100; i++ {
		switch i % 3 {
		case 0:
			ctx[fmt.Sprintf("attr_%d", i)] = fmt.Sprintf("value-%d", i)
		case 1:
			ctx[fmt.Sprintf("attr_%d", i)] = i * 42
		case 2:
			ctx[fmt.Sprintf("attr_%d", i)] = i%2 == 0
		}
	}
	return ctx
}

// Flag configurations from BENCHMARKS.md

const (
	simpleFlagConfig = `{
		"flags": {
			"simple-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true, "off": false }
			}
		}
	}`

	simpleTargetingConfig = `{
		"flags": {
			"targeting-flag": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [{ "==": [{ "var": "tier" }, "premium"] }, "on", "off"]
				}
			}
		}
	}`

	complexTargetingConfig = `{
		"flags": {
			"complex-flag": {
				"state": "ENABLED",
				"defaultVariant": "basic",
				"variants": { "premium": "premium-tier", "standard": "standard-tier", "basic": "basic-tier" },
				"targeting": {
					"if": [
						{ "and": [
							{ "==": [{ "var": "tier" }, "premium"] },
							{ ">": [{ "var": "score" }, 90] }
						]},
						"premium",
						{ "if": [
							{ "or": [
								{ "==": [{ "var": "tier" }, "standard"] },
								{ ">": [{ "var": "score" }, 50] }
							]},
							"standard",
							"basic"
						]}
					]
				}
			}
		}
	}`

	disabledFlagConfig = `{
		"flags": {
			"disabled-flag": {
				"state": "DISABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false }
			}
		}
	}`

	// Custom operator configs
	fractional2Config = `{
		"flags": {
			"frac2-flag": {
				"state": "ENABLED",
				"defaultVariant": "control",
				"variants": { "control": "control", "treatment": "treatment" },
				"targeting": {
					"fractional": [
						[{ "var": "targetingKey" }],
						["control", 50],
						["treatment", 50]
					]
				}
			}
		}
	}`

	fractional8Config = `{
		"flags": {
			"frac8-flag": {
				"state": "ENABLED",
				"defaultVariant": "v1",
				"variants": { "v1":"v1","v2":"v2","v3":"v3","v4":"v4","v5":"v5","v6":"v6","v7":"v7","v8":"v8" },
				"targeting": {
					"fractional": [
						[{ "var": "targetingKey" }],
						["v1", 12], ["v2", 13], ["v3", 12], ["v4", 13],
						["v5", 12], ["v6", 13], ["v7", 12], ["v8", 13]
					]
				}
			}
		}
	}`

	semverEqConfig = `{
		"flags": {
			"semver-eq-flag": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [
						{ "sem_ver": [{ "var": "version" }, "=", "1.2.3"] },
						"on", "off"
					]
				}
			}
		}
	}`

	semverRangeConfig = `{
		"flags": {
			"semver-range-flag": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [
						{ "sem_ver": [{ "var": "version" }, "^", "1.2.0"] },
						"on", "off"
					]
				}
			}
		}
	}`

	startsWithConfig = `{
		"flags": {
			"starts-flag": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [
						{ "starts_with": [{ "var": "email" }, "admin"] },
						"on", "off"
					]
				}
			}
		}
	}`

	endsWithConfig = `{
		"flags": {
			"ends-flag": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [
						{ "ends_with": [{ "var": "email" }, "@example.com"] },
						"on", "off"
					]
				}
			}
		}
	}`
)

func newBenchEvaluator(b *testing.B) *FlagEvaluator {
	b.Helper()
	e, err := NewFlagEvaluator(WithPermissiveValidation())
	if err != nil {
		b.Fatalf("failed to create evaluator: %v", err)
	}
	b.Cleanup(func() { e.Close() })
	return e
}

// ====================================================================
// E1-E11: Core Evaluation Matrix
// ====================================================================

// E1: Simple flag, empty context (baseline)
func BenchmarkE1_SimpleFlag_EmptyContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleFlagConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("simple-flag", emptyCtx)
	}
}

// E2: Simple flag, small context (serialization overhead)
func BenchmarkE2_SimpleFlag_SmallContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleFlagConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("simple-flag", smallCtx)
	}
}

// E3: Simple flag, large context (serialization cost dominance)
func BenchmarkE3_SimpleFlag_LargeContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleFlagConfig)
	ctx := makeLargeCtx()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("simple-flag", ctx)
	}
}

// E4: Simple targeting, small context
func BenchmarkE4_SimpleTargeting_SmallContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("targeting-flag", smallCtx)
	}
}

// E5: Simple targeting, large context
func BenchmarkE5_SimpleTargeting_LargeContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)
	ctx := makeLargeCtx()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("targeting-flag", ctx)
	}
}

// E6: Complex targeting, small context
func BenchmarkE6_ComplexTargeting_SmallContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(complexTargetingConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("complex-flag", smallCtx)
	}
}

// E7: Complex targeting, large context
func BenchmarkE7_ComplexTargeting_LargeContext(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(complexTargetingConfig)
	ctx := makeLargeCtx()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("complex-flag", ctx)
	}
}

// E8: Targeting match
func BenchmarkE8_TargetingMatch(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("targeting-flag", smallCtx) // tier=premium matches
	}
}

// E9: Targeting no-match (default)
func BenchmarkE9_TargetingNoMatch(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)
	ctx := map[string]interface{}{
		"targetingKey": "user-123",
		"tier":         "free",
		"role":         "user",
		"region":       "eu-west",
		"score":        10,
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("targeting-flag", ctx)
	}
}

// E10: Disabled flag
func BenchmarkE10_DisabledFlag(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(disabledFlagConfig)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("disabled-flag", emptyCtx)
	}
}

// E11: Missing flag
func BenchmarkE11_MissingFlag(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(`{"flags": {}}`)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("nonexistent", emptyCtx)
	}
}

// ====================================================================
// O1-O6: Custom Operator Benchmarks
// ====================================================================

// O1: Fractional (2 buckets)
func BenchmarkO1_Fractional2(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(fractional2Config)
	ctx := map[string]interface{}{"targetingKey": "user-123"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("frac2-flag", ctx)
	}
}

// O2: Fractional (8 buckets)
func BenchmarkO2_Fractional8(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(fractional8Config)
	ctx := map[string]interface{}{"targetingKey": "user-123"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("frac8-flag", ctx)
	}
}

// O3: Semver equality
func BenchmarkO3_SemverEquality(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(semverEqConfig)
	ctx := map[string]interface{}{"version": "1.2.3"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("semver-eq-flag", ctx)
	}
}

// O4: Semver range
func BenchmarkO4_SemverRange(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(semverRangeConfig)
	ctx := map[string]interface{}{"version": "1.5.0"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("semver-range-flag", ctx)
	}
}

// O5: starts_with
func BenchmarkO5_StartsWith(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(startsWithConfig)
	ctx := map[string]interface{}{"email": "admin@example.com"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("starts-flag", ctx)
	}
}

// O6: ends_with
func BenchmarkO6_EndsWith(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(endsWithConfig)
	ctx := map[string]interface{}{"email": "user@example.com"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("ends-flag", ctx)
	}
}

// ====================================================================
// S1-S5: State Management Benchmarks
// ====================================================================

// S1: Update state (5 flags)
func BenchmarkS1_UpdateState_5Flags(b *testing.B) {
	e := newBenchEvaluator(b)
	config := generateFlagConfig(5)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.UpdateState(config)
	}
}

// S2: Update state (50 flags)
func BenchmarkS2_UpdateState_50Flags(b *testing.B) {
	e := newBenchEvaluator(b)
	config := generateFlagConfig(50)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.UpdateState(config)
	}
}

// S3: Update state (200 flags)
func BenchmarkS3_UpdateState_200Flags(b *testing.B) {
	e := newBenchEvaluator(b)
	config := generateFlagConfig(200)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.UpdateState(config)
	}
}

// S4: Update state (no change — same config twice)
func BenchmarkS4_UpdateState_NoChange(b *testing.B) {
	e := newBenchEvaluator(b)
	config := generateFlagConfig(100)
	e.UpdateState(config) // first call
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.UpdateState(config) // no changes
	}
}

// S5: Update state (1 flag changed in 100)
func BenchmarkS5_UpdateState_1Changed(b *testing.B) {
	e := newBenchEvaluator(b)
	config1 := generateFlagConfig(100)
	e.UpdateState(config1)
	// Change just one flag's variant value
	config2 := generateFlagConfigWithVariant(100, 50, "modified")
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if i%2 == 0 {
			e.UpdateState(config2)
		} else {
			e.UpdateState(config1)
		}
	}
}

// ====================================================================
// C1-C6: Concurrency Benchmarks
// ====================================================================

// C1: Simple flag, single goroutine (baseline)
func BenchmarkC1_SingleGoroutine(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleFlagConfig)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("simple-flag", emptyCtx)
	}
}

// C2: Simple flag, 4 goroutines
func BenchmarkC2_4Goroutines(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleFlagConfig)
	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			e.EvaluateFlag("simple-flag", emptyCtx)
		}
	})
}

// C3: Simple flag, 8 goroutines
func BenchmarkC3_8Goroutines(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleFlagConfig)
	b.SetParallelism(8)
	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			e.EvaluateFlag("simple-flag", emptyCtx)
		}
	})
}

// C4: Targeting flag, 4 goroutines
func BenchmarkC4_Targeting_4Goroutines(b *testing.B) {
	e := newBenchEvaluator(b)
	e.UpdateState(simpleTargetingConfig)
	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			e.EvaluateFlag("targeting-flag", smallCtx)
		}
	})
}

// C5: Mixed workload, 4 goroutines (realistic production mix)
func BenchmarkC5_MixedWorkload(b *testing.B) {
	config := `{
		"flags": {
			"static-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true, "off": false }
			},
			"targeting-flag": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false },
				"targeting": { "if": [{ "==": [{ "var": "tier" }, "premium"] }, "on", "off"] }
			},
			"disabled-flag": {
				"state": "DISABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false }
			}
		}
	}`

	e := newBenchEvaluator(b)
	e.UpdateState(config)
	flags := []string{"static-flag", "targeting-flag", "disabled-flag"}

	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		i := 0
		for pb.Next() {
			e.EvaluateFlag(flags[i%3], smallCtx)
			i++
		}
	})
}

// C6: Read/write contention (evaluate + update_state concurrently)
func BenchmarkC6_ReadWriteContention(b *testing.B) {
	config1 := `{
		"flags": {
			"flag-a": { "state": "ENABLED", "defaultVariant": "on", "variants": { "on": true } }
		}
	}`
	config2 := `{
		"flags": {
			"flag-a": { "state": "ENABLED", "defaultVariant": "off", "variants": { "off": false } }
		}
	}`

	e := newBenchEvaluator(b)
	e.UpdateState(config1)

	// Writer goroutine
	done := make(chan struct{})
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		i := 0
		for {
			select {
			case <-done:
				return
			default:
				if i%2 == 0 {
					e.UpdateState(config1)
				} else {
					e.UpdateState(config2)
				}
				i++
			}
		}
	}()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		e.EvaluateFlag("flag-a", emptyCtx)
	}
	b.StopTimer()

	close(done)
	wg.Wait()
}

// ====================================================================
// T: Throughput benchmarks — 1000 evaluations per op across N goroutines.
// Exposes mutex contention by measuring aggregate throughput scaling.
// ====================================================================

const throughputOps = 1000

// T1: Pre-evaluated (static) flag, 1 goroutine baseline
func BenchmarkT1_PreEval_1G(b *testing.B) {
	benchThroughput(b, 1, "static-flag", emptyCtx, mixedConfig)
}

// T2: Pre-evaluated (static) flag, 4 goroutines
func BenchmarkT2_PreEval_4G(b *testing.B) {
	benchThroughput(b, 4, "static-flag", emptyCtx, mixedConfig)
}

// T3: Pre-evaluated (static) flag, 16 goroutines
func BenchmarkT3_PreEval_16G(b *testing.B) {
	benchThroughput(b, 16, "static-flag", emptyCtx, mixedConfig)
}

// T4: Targeting flag, 1 goroutine baseline
func BenchmarkT4_Targeting_1G(b *testing.B) {
	benchThroughput(b, 1, "targeting-flag", smallCtx, mixedConfig)
}

// T5: Targeting flag, 4 goroutines
func BenchmarkT5_Targeting_4G(b *testing.B) {
	benchThroughput(b, 4, "targeting-flag", smallCtx, mixedConfig)
}

// T6: Targeting flag, 16 goroutines
func BenchmarkT6_Targeting_16G(b *testing.B) {
	benchThroughput(b, 16, "targeting-flag", smallCtx, mixedConfig)
}

// T7: Mixed workload (static + targeting + disabled), 1 goroutine
func BenchmarkT7_Mixed_1G(b *testing.B) {
	benchThroughputMixed(b, 1)
}

// T8: Mixed workload, 4 goroutines
func BenchmarkT8_Mixed_4G(b *testing.B) {
	benchThroughputMixed(b, 4)
}

// T9: Mixed workload, 16 goroutines
func BenchmarkT9_Mixed_16G(b *testing.B) {
	benchThroughputMixed(b, 16)
}

const mixedConfig = `{
	"flags": {
		"static-flag": {
			"state": "ENABLED",
			"defaultVariant": "on",
			"variants": { "on": true, "off": false }
		},
		"targeting-flag": {
			"state": "ENABLED",
			"defaultVariant": "off",
			"variants": { "on": true, "off": false },
			"targeting": { "if": [{ "==": [{ "var": "tier" }, "premium"] }, "on", "off"] }
		},
		"disabled-flag": {
			"state": "DISABLED",
			"defaultVariant": "off",
			"variants": { "on": true, "off": false }
		}
	}
}`

// benchThroughput runs throughputOps evaluations split across n goroutines per b.N op.
func benchThroughput(b *testing.B, goroutines int, flagKey string, ctx map[string]interface{}, config string) {
	b.Helper()
	e := newBenchEvaluator(b)
	e.UpdateState(config)

	opsPerG := throughputOps / goroutines

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var wg sync.WaitGroup
		wg.Add(goroutines)
		for g := 0; g < goroutines; g++ {
			go func() {
				defer wg.Done()
				for j := 0; j < opsPerG; j++ {
					e.EvaluateFlag(flagKey, ctx)
				}
			}()
		}
		wg.Wait()
	}
}

// benchThroughputMixed runs throughputOps evaluations of mixed flag types across n goroutines.
func benchThroughputMixed(b *testing.B, goroutines int) {
	b.Helper()
	e := newBenchEvaluator(b)
	e.UpdateState(mixedConfig)

	flags := []struct {
		key string
		ctx map[string]interface{}
	}{
		{"static-flag", emptyCtx},
		{"targeting-flag", smallCtx},
		{"disabled-flag", emptyCtx},
	}

	opsPerG := throughputOps / goroutines

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var wg sync.WaitGroup
		wg.Add(goroutines)
		for g := 0; g < goroutines; g++ {
			go func(gIdx int) {
				defer wg.Done()
				for j := 0; j < opsPerG; j++ {
					f := flags[(gIdx+j)%3]
					e.EvaluateFlag(f.key, f.ctx)
				}
			}(g)
		}
		wg.Wait()
	}
}

// ====================================================================
// Helpers
// ====================================================================

func generateFlagConfig(n int) string {
	var buf []byte
	buf = append(buf, `{"flags":{`...)
	for i := 0; i < n; i++ {
		if i > 0 {
			buf = append(buf, ',')
		}
		buf = append(buf, fmt.Sprintf(`"flag-%d":{"state":"ENABLED","defaultVariant":"on","variants":{"on":true,"off":false}}`, i)...)
	}
	buf = append(buf, `}}`...)
	return string(buf)
}

func generateFlagConfigWithVariant(n, changedIdx int, variant string) string {
	var buf []byte
	buf = append(buf, `{"flags":{`...)
	for i := 0; i < n; i++ {
		if i > 0 {
			buf = append(buf, ',')
		}
		if i == changedIdx {
			buf = append(buf, fmt.Sprintf(`"flag-%d":{"state":"ENABLED","defaultVariant":"on","variants":{"on":"%s","off":false}}`, i, variant)...)
		} else {
			buf = append(buf, fmt.Sprintf(`"flag-%d":{"state":"ENABLED","defaultVariant":"on","variants":{"on":true,"off":false}}`, i)...)
		}
	}
	buf = append(buf, `}}`...)
	return string(buf)
}
