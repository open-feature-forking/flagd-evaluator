package evaluator

import (
	"fmt"
	"sync"
	"testing"
)

func newTestEvaluator(t *testing.T) *FlagEvaluator {
	t.Helper()
	e, err := NewFlagEvaluator(WithPermissiveValidation())
	if err != nil {
		t.Fatalf("failed to create evaluator: %v", err)
	}
	t.Cleanup(func() { e.Close() })
	return e
}

func TestSimpleBooleanFlag(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"simple-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": {
					"on": true,
					"off": false
				}
			}
		}
	}`

	result, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}
	if !result.Success {
		t.Fatalf("UpdateState not successful: %s", result.Error)
	}
	assertContains(t, result.ChangedFlags, "simple-flag")

	evalResult, err := e.EvaluateFlag("simple-flag", map[string]interface{}{})
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, true, evalResult.Value)
	assertEqual(t, "on", evalResult.Variant)
	assertEqual(t, "STATIC", evalResult.Reason)
	if evalResult.IsError() {
		t.Errorf("expected no error, got %s: %s", evalResult.ErrorCode, evalResult.ErrorMessage)
	}
}

func TestStringFlag(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"color-flag": {
				"state": "ENABLED",
				"defaultVariant": "red",
				"variants": {
					"red": "red",
					"blue": "blue",
					"green": "green"
				}
			}
		}
	}`

	result, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}
	if !result.Success {
		t.Fatalf("UpdateState not successful: %s", result.Error)
	}

	evalResult, err := e.EvaluateFlag("color-flag", map[string]interface{}{})
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, "red", evalResult.Value)
	assertEqual(t, "red", evalResult.Variant)
}

func TestTargetingRule(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"user-flag": {
				"state": "ENABLED",
				"defaultVariant": "default",
				"variants": {
					"default": false,
					"premium": true
				},
				"targeting": {
					"if": [
						{
							"==": [
								{ "var": "email" },
								"premium@example.com"
							]
						},
						"premium",
						null
					]
				}
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	// Matching context
	ctx := map[string]interface{}{"email": "premium@example.com"}
	result, err := e.EvaluateFlag("user-flag", ctx)
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, true, result.Value)
	assertEqual(t, "premium", result.Variant)
	assertEqual(t, "TARGETING_MATCH", result.Reason)

	// Non-matching context
	ctx = map[string]interface{}{"email": "regular@example.com"}
	result, err = e.EvaluateFlag("user-flag", ctx)
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, false, result.Value)
	assertEqual(t, "default", result.Variant)
}

func TestFlagNotFound(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{"flags": {}}`
	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	result, err := e.EvaluateFlag("nonexistent-flag", map[string]interface{}{})
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, "FLAG_NOT_FOUND", result.Reason)
}

func TestDisabledFlag(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"disabled-flag": {
				"state": "DISABLED",
				"defaultVariant": "off",
				"variants": {
					"on": true,
					"off": false
				}
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	result, err := e.EvaluateFlag("disabled-flag", map[string]interface{}{})
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	if result.Value != nil {
		t.Errorf("expected nil value for disabled flag, got %v", result.Value)
	}
	assertEqual(t, "DISABLED", result.Reason)
}

func TestNumericFlag(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"number-flag": {
				"state": "ENABLED",
				"defaultVariant": "default",
				"variants": {
					"default": 42,
					"large": 1000
				}
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	result, err := e.EvaluateFlag("number-flag", map[string]interface{}{})
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	// JSON numbers unmarshal as float64
	assertEqual(t, float64(42), result.Value)
}

func TestContextEnrichment(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"targeting-key-flag": {
				"state": "ENABLED",
				"defaultVariant": "default",
				"variants": {
					"default": "unknown",
					"known": "known-user"
				},
				"targeting": {
					"if": [
						{
							"!=": [
								{ "var": "targetingKey" },
								""
							]
						},
						"known",
						null
					]
				}
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	ctx := map[string]interface{}{"targetingKey": "user-123"}
	result, err := e.EvaluateFlag("targeting-key-flag", ctx)
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, "known-user", result.Value)
	assertEqual(t, "TARGETING_MATCH", result.Reason)
}

func TestUpdateStateChangedFlags(t *testing.T) {
	e := newTestEvaluator(t)

	// Initial config
	config1 := `{
		"flags": {
			"flag-a": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true }
			}
		}
	}`
	result1, err := e.UpdateState(config1)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}
	if !result1.Success {
		t.Fatalf("UpdateState not successful")
	}
	assertContains(t, result1.ChangedFlags, "flag-a")

	// Update with changed + new flags
	config2 := `{
		"flags": {
			"flag-a": {
				"state": "DISABLED",
				"defaultVariant": "off",
				"variants": { "off": false }
			},
			"flag-b": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true }
			}
		}
	}`
	result2, err := e.UpdateState(config2)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}
	if !result2.Success {
		t.Fatalf("UpdateState not successful")
	}
	assertContains(t, result2.ChangedFlags, "flag-a")
	assertContains(t, result2.ChangedFlags, "flag-b")
}

func TestRequiredContextKeys(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"targeted-flag": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [
						{ "==": [{ "var": "email" }, "admin@example.com"] },
						"on", "off"
					]
				}
			},
			"static-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true }
			}
		}
	}`

	result, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}
	if !result.Success {
		t.Fatalf("UpdateState not successful")
	}

	// Should have required context keys for the targeted flag
	if result.RequiredContextKeys == nil {
		t.Fatal("RequiredContextKeys is nil")
	}
	keys, ok := result.RequiredContextKeys["targeted-flag"]
	if !ok {
		t.Fatal("targeted-flag not in RequiredContextKeys")
	}
	assertContains(t, keys, "email")
	assertContains(t, keys, "targetingKey")

	// Static flags should not be in required context keys
	if _, ok := result.RequiredContextKeys["static-flag"]; ok {
		t.Error("static-flag should not have required context keys")
	}
}

func TestFlagIndices(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"flagB": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true }
			},
			"flagA": {
				"state": "ENABLED",
				"defaultVariant": "off",
				"variants": { "off": false }
			}
		}
	}`

	result, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}
	if !result.Success {
		t.Fatalf("UpdateState not successful")
	}

	if result.FlagIndices == nil {
		t.Fatal("FlagIndices is nil")
	}
	if _, ok := result.FlagIndices["flagA"]; !ok {
		t.Fatal("flagA not in FlagIndices")
	}
	if _, ok := result.FlagIndices["flagB"]; !ok {
		t.Fatal("flagB not in FlagIndices")
	}
	// Indices should be in sorted order
	assertEqual(t, uint32(0), result.FlagIndices["flagA"])
	assertEqual(t, uint32(1), result.FlagIndices["flagB"])
}

func TestFilteredContextEvaluation(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"email-flag": {
				"state": "ENABLED",
				"defaultVariant": "default",
				"variants": { "default": false, "premium": true },
				"targeting": {
					"if": [
						{ "==": [{ "var": "email" }, "admin@example.com"] },
						"premium", null
					]
				}
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	// Large context — only "email" matters
	ctx := map[string]interface{}{
		"targetingKey": "user-123",
		"email":        "admin@example.com",
		"name":         "Admin User",
		"age":          30,
		"country":      "US",
		"tier":         "premium",
		"department":   "engineering",
	}

	result, err := e.EvaluateFlag("email-flag", ctx)
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, true, result.Value)
	assertEqual(t, "premium", result.Variant)
	assertEqual(t, "TARGETING_MATCH", result.Reason)

	// Non-matching email
	ctx2 := map[string]interface{}{
		"targetingKey": "user-456",
		"email":        "regular@example.com",
		"name":         "Regular User",
		"age":          25,
	}
	result, err = e.EvaluateFlag("email-flag", ctx2)
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, false, result.Value)
	assertEqual(t, "default", result.Variant)
}

func TestPreEvaluatedCache(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"static-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true, "off": false }
			},
			"disabled-flag": {
				"state": "DISABLED",
				"defaultVariant": "on",
				"variants": { "on": true, "off": false }
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	// These should be served from pre-evaluated cache
	ctx := map[string]interface{}{
		"targetingKey": "user-1",
		"anything":     "value",
	}

	result, err := e.EvaluateFlag("static-flag", ctx)
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	assertEqual(t, true, result.Value)
	assertEqual(t, "STATIC", result.Reason)

	result, err = e.EvaluateFlag("disabled-flag", ctx)
	if err != nil {
		t.Fatalf("EvaluateFlag failed: %v", err)
	}
	if result.Value != nil {
		t.Errorf("expected nil value for disabled flag, got %v", result.Value)
	}
	assertEqual(t, "DISABLED", result.Reason)
}

func TestConcurrentAccess(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"concurrent-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true, "off": false },
				"targeting": {
					"if": [
						{ "==": [{ "var": "user" }, "admin"] },
						"on", "off"
					]
				}
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	var wg sync.WaitGroup
	errs := make(chan error, 20)

	for i := 0; i < 20; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			ctx := map[string]interface{}{"user": "admin"}
			result, err := e.EvaluateFlag("concurrent-flag", ctx)
			if err != nil {
				errs <- err
				return
			}
			if result.Value != true {
				errs <- fmt.Errorf("goroutine %d: expected true, got %v", i, result.Value)
			}
		}(i)
	}

	wg.Wait()
	close(errs)

	for err := range errs {
		t.Errorf("concurrent error: %v", err)
	}
}

func TestTypedEvaluators(t *testing.T) {
	e := newTestEvaluator(t)

	config := `{
		"flags": {
			"bool-flag": {
				"state": "ENABLED",
				"defaultVariant": "on",
				"variants": { "on": true, "off": false }
			},
			"string-flag": {
				"state": "ENABLED",
				"defaultVariant": "hello",
				"variants": { "hello": "world" }
			},
			"int-flag": {
				"state": "ENABLED",
				"defaultVariant": "val",
				"variants": { "val": 42 }
			},
			"float-flag": {
				"state": "ENABLED",
				"defaultVariant": "val",
				"variants": { "val": 3.14 }
			}
		}
	}`

	_, err := e.UpdateState(config)
	if err != nil {
		t.Fatalf("UpdateState failed: %v", err)
	}

	ctx := map[string]interface{}{}

	if v := e.EvaluateBool("bool-flag", ctx, false); v != true {
		t.Errorf("EvaluateBool: expected true, got %v", v)
	}
	if v := e.EvaluateString("string-flag", ctx, "default"); v != "world" {
		t.Errorf("EvaluateString: expected 'world', got %v", v)
	}
	if v := e.EvaluateInt("int-flag", ctx, 0); v != 42 {
		t.Errorf("EvaluateInt: expected 42, got %v", v)
	}
	if v := e.EvaluateFloat("float-flag", ctx, 0); v != 3.14 {
		t.Errorf("EvaluateFloat: expected 3.14, got %v", v)
	}

	// Test defaults for missing flags
	if v := e.EvaluateBool("missing", ctx, true); v != true {
		t.Errorf("EvaluateBool default: expected true, got %v", v)
	}
	if v := e.EvaluateString("missing", ctx, "fallback"); v != "fallback" {
		t.Errorf("EvaluateString default: expected 'fallback', got %v", v)
	}
	if v := e.EvaluateInt("missing", ctx, 99); v != 99 {
		t.Errorf("EvaluateInt default: expected 99, got %v", v)
	}
	if v := e.EvaluateFloat("missing", ctx, 1.5); v != 1.5 {
		t.Errorf("EvaluateFloat default: expected 1.5, got %v", v)
	}
}

// TestGenerationGuard exercises the race between cache.Load() and pool acquire.
//
// Without the generation check, this sequence causes wrong results:
//   1. Goroutine loads cache snap V1 (flag indices: probe=0, padA=1, padB=2)
//   2. UpdateState swaps to V2 (flag indices shift: padC=0, padD=1, probe=2)
//   3. Goroutine gets V2 instance but uses V1 index 0 → evaluates padC instead of probe
//
// The test alternates between two configs with different flag sets, causing
// indices to shift. Evaluator goroutines verify the result is always valid
// for the "probe" flag and never a value leaked from a padding flag.
func TestGenerationGuard(t *testing.T) {
	// Small pool to increase contention window
	e, err := NewFlagEvaluator(WithPermissiveValidation(), WithPoolSize(2))
	if err != nil {
		t.Fatalf("failed to create evaluator: %v", err)
	}
	t.Cleanup(func() { e.Close() })

	// Config A: padA and padB occupy low indices, probe is at a higher index.
	// probe targets on "tier" == "premium" → variant "yes" (value "AAA")
	configA := `{
		"flags": {
			"aaa-pad-1": {
				"state": "ENABLED",
				"defaultVariant": "v",
				"variants": { "v": "PADDING_A1" }
			},
			"aaa-pad-2": {
				"state": "ENABLED",
				"defaultVariant": "v",
				"variants": { "v": "PADDING_A2" }
			},
			"probe": {
				"state": "ENABLED",
				"defaultVariant": "no",
				"variants": { "yes": "AAA", "no": "default-A" },
				"targeting": {
					"if": [{ "==": [{ "var": "tier" }, "premium"] }, "yes", null]
				}
			}
		}
	}`

	// Config B: different padding flags shift indices. probe should return "BBB".
	configB := `{
		"flags": {
			"zzz-pad-3": {
				"state": "ENABLED",
				"defaultVariant": "v",
				"variants": { "v": "PADDING_B3" }
			},
			"zzz-pad-4": {
				"state": "ENABLED",
				"defaultVariant": "v",
				"variants": { "v": "PADDING_B4" }
			},
			"zzz-pad-5": {
				"state": "ENABLED",
				"defaultVariant": "v",
				"variants": { "v": "PADDING_B5" }
			},
			"probe": {
				"state": "ENABLED",
				"defaultVariant": "no",
				"variants": { "yes": "BBB", "no": "default-B" },
				"targeting": {
					"if": [{ "==": [{ "var": "tier" }, "premium"] }, "yes", null]
				}
			}
		}
	}`

	// Valid results for "probe" across either config
	validValues := map[interface{}]bool{
		"AAA":       true,
		"BBB":       true,
		"default-A": true,
		"default-B": true,
	}

	// Initialize with config A
	if _, err := e.UpdateState(configA); err != nil {
		t.Fatalf("initial UpdateState failed: %v", err)
	}

	const (
		numEvaluators  = 8
		numUpdates     = 50
		evalsPerUpdate = 20
	)

	var wg sync.WaitGroup
	stop := make(chan struct{})
	errs := make(chan string, numEvaluators*numUpdates*evalsPerUpdate)

	// Evaluator goroutines: continuously evaluate "probe" and verify results
	for g := 0; g < numEvaluators; g++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			ctx := map[string]interface{}{"tier": "premium", "targetingKey": "user-1"}
			for {
				select {
				case <-stop:
					return
				default:
				}
				result, err := e.EvaluateFlag("probe", ctx)
				if err != nil {
					errs <- fmt.Sprintf("goroutine %d: EvaluateFlag error: %v", id, err)
					continue
				}
				if !validValues[result.Value] {
					errs <- fmt.Sprintf(
						"goroutine %d: INVALID value %q (variant=%s reason=%s) — possible stale index",
						id, result.Value, result.Variant, result.Reason,
					)
				}
			}
		}(g)
	}

	// Updater: rapidly alternate configs to maximize the race window
	for i := 0; i < numUpdates; i++ {
		if i%2 == 0 {
			e.UpdateState(configB)
		} else {
			e.UpdateState(configA)
		}
	}

	close(stop)
	wg.Wait()
	close(errs)

	for msg := range errs {
		t.Error(msg)
	}
}

// ---- Test helpers ----

func assertEqual(t *testing.T, expected, actual interface{}) {
	t.Helper()
	if expected != actual {
		t.Errorf("expected %v (%T), got %v (%T)", expected, expected, actual, actual)
	}
}

func assertContains(t *testing.T, slice interface{}, item interface{}) {
	t.Helper()
	switch s := slice.(type) {
	case []string:
		for _, v := range s {
			if v == item {
				return
			}
		}
		t.Errorf("expected %v to contain %v", s, item)
	default:
		t.Errorf("assertContains: unsupported slice type %T", slice)
	}
}
