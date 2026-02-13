package evaluator

import (
	"fmt"
	"sort"
	"sync"
	"testing"
	"time"
)

// Deeply nested targeting rule that checks many variables across multiple
// levels of if/and/or. Each evaluation triggers more WASM memory allocations
// for variable lookups and intermediate results than complexTargetingConfig.
const bigTargetingConfig = `{
	"flags": {
		"big-flag": {
			"state": "ENABLED",
			"defaultVariant": "none",
			"variants": {
				"premium": "premium-tier",
				"standard": "standard-tier",
				"basic": "basic-tier",
				"none": "no-tier"
			},
			"targeting": {
				"if": [
					{ "and": [
						{ "==": [{ "var": "tier" }, "premium"] },
						{ ">": [{ "var": "score" }, 90] },
						{ "==": [{ "var": "region" }, "us-east"] },
						{ "in": [{ "var": "role" }, ["admin", "superadmin"]] }
					]},
					"premium",
					{ "if": [
						{ "or": [
							{ "and": [
								{ "==": [{ "var": "tier" }, "standard"] },
								{ ">": [{ "var": "score" }, 50] }
							]},
							{ "and": [
								{ "==": [{ "var": "department" }, "engineering"] },
								{ ">=": [{ "var": "experience" }, 5] }
							]},
							{ "and": [
								{ "==": [{ "var": "country" }, "US"] },
								{ ">": [{ "var": "level" }, 3] }
							]}
						]},
						"standard",
						{ "if": [
							{ "or": [
								{ ">": [{ "var": "score" }, 20] },
								{ "!=": [{ "var": "plan" }, "free"] }
							]},
							"basic",
							null
						]}
					]}
				]
			}
		}
	}
}`

// generateBigStoreConfig creates a flag store with n padding flags (mix of static,
// targeting, disabled) plus the big targeting flag. This bloats WASM linear memory
// and increases per-evaluation overhead from the larger flag index.
func generateBigStoreConfig(n int) string {
	var buf []byte
	buf = append(buf, `{"flags":{`...)

	// The big targeting flag we'll actually evaluate
	buf = append(buf, `"big-flag":{
		"state":"ENABLED","defaultVariant":"none",
		"variants":{"premium":"premium-tier","standard":"standard-tier","basic":"basic-tier","none":"no-tier"},
		"targeting":{"if":[
			{"and":[{"==":[{"var":"tier"},"premium"]},{">":[{"var":"score"},90]},{"==":[{"var":"region"},"us-east"]},{"in":[{"var":"role"},["admin","superadmin"]]}]},
			"premium",
			{"if":[
				{"or":[{"and":[{"==":[{"var":"tier"},"standard"]},{">":[{"var":"score"},50]}]},{"and":[{"==":[{"var":"department"},"engineering"]},{">=":[{"var":"experience"},5]}]},{"and":[{"==":[{"var":"country"},"US"]},{">":[{"var":"level"},3]}]}]},
				"standard",
				{"if":[{"or":[{">":[{"var":"score"},20]},{"!=":[{"var":"plan"},"free"]}]},"basic",null]}
			]}
		]}}`...)

	// Padding flags: mix of static, targeting, and disabled
	for i := 0; i < n; i++ {
		buf = append(buf, ',')
		switch i % 3 {
		case 0: // static
			buf = append(buf, fmt.Sprintf(`"pad-flag-%d":{"state":"ENABLED","defaultVariant":"on","variants":{"on":true,"off":false}}`, i)...)
		case 1: // targeting
			buf = append(buf, fmt.Sprintf(`"pad-flag-%d":{"state":"ENABLED","defaultVariant":"off","variants":{"on":true,"off":false},"targeting":{"if":[{"==":[{"var":"tier"},"premium"]},"on","off"]}}`, i)...)
		case 2: // disabled
			buf = append(buf, fmt.Sprintf(`"pad-flag-%d":{"state":"DISABLED","defaultVariant":"off","variants":{"on":true,"off":false}}`, i)...)
		}
	}
	buf = append(buf, `}}`...)
	return string(buf)
}

// TestP99LatencyStability runs sustained sequential evaluations across 10-second
// time windows for 2 minutes per scenario, asserting that p99 latency does not
// degrade over time. This catches GC pressure issues where per-evaluation
// allocations accumulate and cause GC pauses to grow.
func TestP99LatencyStability(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping latency stability test in short mode")
	}

	const (
		windowDuration = 10 * time.Second
		numWindows     = 12 // 12 × 10s = 2 minutes per sub-test
	)

	type subTest struct {
		name    string
		config  string
		flagKey string
		makeCtx func(i int) map[string]interface{}
	}

	tests := []subTest{
		{
			name:    "SmallContext",
			config:  complexTargetingConfig,
			flagKey: "complex-flag",
			makeCtx: func(i int) map[string]interface{} {
				return map[string]interface{}{
					"targetingKey": fmt.Sprintf("user-%d", i),
					"tier":         "premium",
					"role":         "admin",
					"region":       "us-east",
					"score":        i % 100,
				}
			},
		},
		{
			name:    "LargeContext",
			config:  complexTargetingConfig,
			flagKey: "complex-flag",
			makeCtx: func(i int) map[string]interface{} {
				ctx := map[string]interface{}{
					"targetingKey": fmt.Sprintf("user-%d", i),
					"tier":         "premium",
					"role":         "admin",
					"region":       "us-east",
					"score":        i % 100,
				}
				for j := 0; j < 50; j++ {
					ctx[fmt.Sprintf("attr_%d", j)] = fmt.Sprintf("value-%d-%d", i, j)
				}
				return ctx
			},
		},
		{
			name:    "BigTargeting",
			config:  bigTargetingConfig,
			flagKey: "big-flag",
			makeCtx: func(i int) map[string]interface{} {
				ctx := map[string]interface{}{
					"targetingKey": fmt.Sprintf("user-%d", i),
					"tier":         "premium",
					"role":         "admin",
					"region":       "us-east",
					"score":        i % 100,
					"department":   "engineering",
					"experience":   i % 15,
					"country":      "US",
					"level":        i % 10,
					"plan":         "pro",
				}
				for j := 0; j < 50; j++ {
					ctx[fmt.Sprintf("attr_%d", j)] = fmt.Sprintf("value-%d-%d", i, j)
				}
				return ctx
			},
		},
		{
			name:    "BigStore",
			config:  generateBigStoreConfig(500),
			flagKey: "big-flag",
			makeCtx: func(i int) map[string]interface{} {
				ctx := map[string]interface{}{
					"targetingKey": fmt.Sprintf("user-%d", i),
					"tier":         "premium",
					"role":         "admin",
					"region":       "us-east",
					"score":        i % 100,
					"department":   "engineering",
					"experience":   i % 15,
					"country":      "US",
					"level":        i % 10,
					"plan":         "pro",
				}
				for j := 0; j < 50; j++ {
					ctx[fmt.Sprintf("attr_%d", j)] = fmt.Sprintf("value-%d-%d", i, j)
				}
				return ctx
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			e := newTestEvaluator(t)
			_, err := e.UpdateState(tc.config)
			if err != nil {
				t.Fatalf("UpdateState failed: %v", err)
			}

			p99s := make([]time.Duration, numWindows)
			evalCounts := make([]int, numWindows)

			for w := 0; w < numWindows; w++ {
				var latencies []time.Duration
				deadline := time.Now().Add(windowDuration)
				i := 0
				for time.Now().Before(deadline) {
					ctx := tc.makeCtx(w*1_000_000 + i)
					start := time.Now()
					_, err := e.EvaluateFlag(tc.flagKey, ctx)
					latencies = append(latencies, time.Since(start))
					if err != nil {
						t.Fatalf("EvaluateFlag failed: %v", err)
					}
					i++
				}
				p99s[w] = percentile(latencies, 0.99)
				evalCounts[w] = len(latencies)
			}

			for w := range p99s {
				t.Logf("window %2d: p99 = %-12v evals = %d", w, p99s[w], evalCounts[w])
			}

			// Window 0 is warmup; window 1 is baseline
			baseline := p99s[1]
			t.Logf("baseline (window 1): p99 = %v", baseline)

			// Check 1: No window exceeds 3x baseline
			for w := 2; w < numWindows; w++ {
				if p99s[w] > 3*baseline {
					t.Errorf("window %d p99 (%v) exceeds 3x baseline (%v)", w, p99s[w], 3*baseline)
				}
			}

			// Check 2: Last 5+ consecutive windows all above 1.5x = "spike and stay" pattern
			consecutiveAbove := 0
			for w := numWindows - 1; w >= 2; w-- {
				if p99s[w] > baseline+(baseline/2) {
					consecutiveAbove++
				} else {
					break
				}
			}
			if consecutiveAbove >= 5 {
				t.Errorf("last %d consecutive windows all above 1.5x baseline — p99 is not recovering", consecutiveAbove)
			}
		})
	}
}

// TestP99LatencyStabilityConcurrent runs sustained parallel evaluations across
// 10-second time windows for 2 minutes with big targeting + big store.
// This catches issues that only manifest under pool contention + GC pressure.
func TestP99LatencyStabilityConcurrent(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping concurrent latency stability test in short mode")
	}

	const (
		numGoroutines  = 4
		windowDuration = 10 * time.Second
		numWindows     = 12
	)

	e, err := NewFlagEvaluator(WithPermissiveValidation(), WithPoolSize(numGoroutines))
	if err != nil {
		t.Fatalf("failed to create evaluator: %v", err)
	}
	t.Cleanup(func() { e.Close() })

	_, err = e.UpdateState(generateBigStoreConfig(500))
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	p99s := make([]time.Duration, numWindows)
	evalCounts := make([]int, numWindows)

	for w := 0; w < numWindows; w++ {
		var mu sync.Mutex
		var allLatencies []time.Duration

		var wg sync.WaitGroup
		wg.Add(numGoroutines)
		for g := 0; g < numGoroutines; g++ {
			go func(gID int) {
				defer wg.Done()
				var local []time.Duration
				deadline := time.Now().Add(windowDuration)
				i := 0
				for time.Now().Before(deadline) {
					ctx := map[string]interface{}{
						"targetingKey": fmt.Sprintf("user-%d-%d-%d", w, gID, i),
						"tier":         "premium",
						"role":         "admin",
						"region":       "us-east",
						"score":        (gID*100_000 + i) % 100,
						"department":   "engineering",
						"experience":   i % 15,
						"country":      "US",
						"level":        i % 10,
						"plan":         "pro",
					}
					start := time.Now()
					e.EvaluateFlag("big-flag", ctx)
					local = append(local, time.Since(start))
					i++
				}
				mu.Lock()
				allLatencies = append(allLatencies, local...)
				mu.Unlock()
			}(g)
		}
		wg.Wait()
		p99s[w] = percentile(allLatencies, 0.99)
		evalCounts[w] = len(allLatencies)
	}

	for w := range p99s {
		t.Logf("window %2d: p99 = %-12v evals = %d", w, p99s[w], evalCounts[w])
	}

	baseline := p99s[1]
	t.Logf("baseline (window 1): p99 = %v", baseline)

	// Relaxed to 4x for concurrent (contention adds variance)
	for w := 2; w < numWindows; w++ {
		if p99s[w] > 4*baseline {
			t.Errorf("window %d p99 (%v) exceeds 4x baseline (%v)", w, p99s[w], 4*baseline)
		}
	}

	consecutiveAbove := 0
	for w := numWindows - 1; w >= 2; w-- {
		if p99s[w] > baseline+(baseline/2) {
			consecutiveAbove++
		} else {
			break
		}
	}
	if consecutiveAbove >= 5 {
		t.Errorf("last %d consecutive windows all above 1.5x baseline — p99 is not recovering", consecutiveAbove)
	}
}

// percentile returns the p-th percentile from a slice of durations.
// p should be between 0 and 1 (e.g., 0.99 for p99).
func percentile(latencies []time.Duration, p float64) time.Duration {
	sort.Slice(latencies, func(i, j int) bool { return latencies[i] < latencies[j] })
	idx := int(float64(len(latencies)-1) * p)
	return latencies[idx]
}
