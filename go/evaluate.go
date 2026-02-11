package evaluator

import (
	"context"
	"encoding/json"
	"fmt"
	"strconv"
	"strings"
	"time"
)

// EvaluateFlag evaluates a flag and returns the full result.
func (e *FlagEvaluator) EvaluateFlag(flagKey string, ctx map[string]interface{}) (*EvaluationResult, error) {
	return e.evaluateFlag(flagKey, ctx)
}

// EvaluateBool evaluates a boolean flag. Returns defaultValue on error.
func (e *FlagEvaluator) EvaluateBool(flagKey string, ctx map[string]interface{}, defaultValue bool) bool {
	result, err := e.evaluateFlag(flagKey, ctx)
	if err != nil || result.IsError() || result.Value == nil {
		return defaultValue
	}
	if v, ok := result.Value.(bool); ok {
		return v
	}
	return defaultValue
}

// EvaluateString evaluates a string flag. Returns defaultValue on error.
func (e *FlagEvaluator) EvaluateString(flagKey string, ctx map[string]interface{}, defaultValue string) string {
	result, err := e.evaluateFlag(flagKey, ctx)
	if err != nil || result.IsError() || result.Value == nil {
		return defaultValue
	}
	if v, ok := result.Value.(string); ok {
		return v
	}
	return defaultValue
}

// EvaluateInt evaluates an integer flag. Returns defaultValue on error.
func (e *FlagEvaluator) EvaluateInt(flagKey string, ctx map[string]interface{}, defaultValue int64) int64 {
	result, err := e.evaluateFlag(flagKey, ctx)
	if err != nil || result.IsError() || result.Value == nil {
		return defaultValue
	}
	// JSON numbers unmarshal as float64
	if v, ok := result.Value.(float64); ok {
		return int64(v)
	}
	return defaultValue
}

// EvaluateFloat evaluates a float flag. Returns defaultValue on error.
func (e *FlagEvaluator) EvaluateFloat(flagKey string, ctx map[string]interface{}, defaultValue float64) float64 {
	result, err := e.evaluateFlag(flagKey, ctx)
	if err != nil || result.IsError() || result.Value == nil {
		return defaultValue
	}
	if v, ok := result.Value.(float64); ok {
		return v
	}
	return defaultValue
}

// evaluateFlag is the internal evaluation pipeline.
func (e *FlagEvaluator) evaluateFlag(flagKey string, ctx map[string]interface{}) (*EvaluationResult, error) {
	// Load caches atomically (lock-free)
	snap := e.cache.Load()

	// Fast path: pre-evaluated cache hit (static/disabled flags)
	if cached, ok := snap.preEvaluated[flagKey]; ok {
		return cached, nil
	}

	// Acquire an instance from the pool
	inst := <-e.pool
	defer func() { e.pool <- inst }()

	// If an UpdateState completed between cache.Load() and pool acquire,
	// the snap has stale indices. Reload to match the instance's generation.
	if snap.generation != inst.generation {
		snap = e.cache.Load()
		// Re-check pre-eval cache — flag may now be static
		if cached, ok := snap.preEvaluated[flagKey]; ok {
			return cached, nil
		}
	}

	// Determine context serialization strategy
	var contextBytes []byte
	requiredKeys := snap.requiredCtxKey[flagKey]
	if requiredKeys != nil && len(ctx) > 0 {
		contextBytes = serializeFilteredContext(ctx, requiredKeys, flagKey)
	} else if len(ctx) > 0 {
		var err error
		contextBytes, err = json.Marshal(ctx)
		if err != nil {
			return nil, fmt.Errorf("failed to marshal context: %w", err)
		}
	}

	// Evaluate using the instance
	flagIndex, hasIndex := snap.flagIndex[flagKey]
	if hasIndex && inst.evalByIndexFn != nil && requiredKeys != nil {
		return evaluateByIndex(e.ctx, inst, flagIndex, contextBytes)
	}
	return evaluateReusable(e.ctx, inst, flagKey, contextBytes)
}

// evaluateByIndex calls the evaluate_by_index WASM export on a specific instance.
func evaluateByIndex(ctx context.Context, inst *wasmInstance, flagIndex uint32, contextBytes []byte) (result *EvaluationResult, err error) {
	defer func() {
		if r := recover(); r != nil {
			result = nil
			err = fmt.Errorf("WASM panic: %v", r)
		}
	}()

	var contextPtr, contextLen uint32
	if len(contextBytes) > 0 {
		if err := writeToPreallocBuffer(inst.module, inst.contextBufPtr, maxContextSize, contextBytes); err != nil {
			return nil, err
		}
		contextPtr = inst.contextBufPtr
		contextLen = uint32(len(contextBytes))
	}

	results, err := inst.evalByIndexFn.Call(ctx, uint64(flagIndex), uint64(contextPtr), uint64(contextLen))
	if err != nil {
		return nil, fmt.Errorf("evaluate_by_index call failed: %w", err)
	}

	return readEvalResult(ctx, inst, results[0])
}

// evaluateReusable calls the evaluate_reusable WASM export on a specific instance.
func evaluateReusable(ctx context.Context, inst *wasmInstance, flagKey string, contextBytes []byte) (result *EvaluationResult, err error) {
	defer func() {
		if r := recover(); r != nil {
			result = nil
			err = fmt.Errorf("WASM panic: %v", r)
		}
	}()

	flagBytes := []byte(flagKey)
	if err := writeToPreallocBuffer(inst.module, inst.flagKeyBufPtr, maxFlagKeySize, flagBytes); err != nil {
		return nil, fmt.Errorf("flag key too large: %w", err)
	}

	var contextPtr, contextLen uint32
	if len(contextBytes) > 0 {
		if err := writeToPreallocBuffer(inst.module, inst.contextBufPtr, maxContextSize, contextBytes); err != nil {
			return nil, err
		}
		contextPtr = inst.contextBufPtr
		contextLen = uint32(len(contextBytes))
	}

	results, err := inst.evalReusableFn.Call(ctx,
		uint64(inst.flagKeyBufPtr), uint64(len(flagBytes)),
		uint64(contextPtr), uint64(contextLen))
	if err != nil {
		return nil, fmt.Errorf("evaluate_reusable call failed: %w", err)
	}

	return readEvalResult(ctx, inst, results[0])
}

// readEvalResult reads and parses an evaluation result from a packed u64.
func readEvalResult(ctx context.Context, inst *wasmInstance, packed uint64) (*EvaluationResult, error) {
	resultPtr, resultLen := unpackPtrLen(packed)
	resultBytes, err := readFromWasm(inst.module, resultPtr, resultLen)
	if err != nil {
		return nil, fmt.Errorf("failed to read evaluation result: %w", err)
	}
	inst.deallocFn.Call(ctx, uint64(resultPtr), uint64(resultLen))

	result, err := parseEvalResult(resultBytes)
	if err != nil {
		return nil, fmt.Errorf("failed to parse evaluation result: %w", err)
	}
	return result, nil
}

// serializeFilteredContext builds a JSON context with only the required keys,
// plus targetingKey and $flagd enrichment. Uses strings.Builder for performance.
func serializeFilteredContext(ctx map[string]interface{}, requiredKeys map[string]bool, flagKey string) []byte {
	var b strings.Builder
	b.Grow(256)
	b.WriteByte('{')

	first := true
	writeComma := func() {
		if !first {
			b.WriteByte(',')
		}
		first = false
	}

	// Write required keys from context
	for key := range requiredKeys {
		if key == "targetingKey" || key == "$flagd.flagKey" || key == "$flagd.timestamp" {
			continue // handled separately
		}
		val, exists := ctx[key]
		if !exists {
			continue
		}
		writeComma()
		b.WriteByte('"')
		b.WriteString(key)
		b.WriteString(`":`)
		writeJSONValue(&b, val)
	}

	// Always include targetingKey
	writeComma()
	b.WriteString(`"targetingKey":`)
	if tk, ok := ctx["targetingKey"]; ok {
		writeJSONValue(&b, tk)
	} else {
		b.WriteString(`""`)
	}

	// $flagd enrichment
	writeComma()
	b.WriteString(`"$flagd":{"flagKey":"`)
	b.WriteString(flagKey)
	b.WriteString(`","timestamp":`)
	b.WriteString(strconv.FormatInt(time.Now().Unix(), 10))
	b.WriteByte('}')

	b.WriteByte('}')
	return []byte(b.String())
}

// writeJSONValue writes a JSON-encoded value to the builder.
// For simple types it avoids json.Marshal overhead.
func writeJSONValue(b *strings.Builder, val interface{}) {
	switch v := val.(type) {
	case string:
		b.WriteByte('"')
		b.WriteString(escapeJSONString(v))
		b.WriteByte('"')
	case bool:
		if v {
			b.WriteString("true")
		} else {
			b.WriteString("false")
		}
	case float64:
		b.WriteString(strconv.FormatFloat(v, 'f', -1, 64))
	case float32:
		b.WriteString(strconv.FormatFloat(float64(v), 'f', -1, 32))
	case int:
		b.WriteString(strconv.Itoa(v))
	case int64:
		b.WriteString(strconv.FormatInt(v, 10))
	case nil:
		b.WriteString("null")
	default:
		// Fall back to json.Marshal for complex types
		data, err := json.Marshal(v)
		if err != nil {
			b.WriteString("null")
			return
		}
		b.Write(data)
	}
}

// escapeJSONString escapes special characters in a JSON string value.
func escapeJSONString(s string) string {
	// Fast path: no escaping needed for most strings
	needsEscape := false
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c == '"' || c == '\\' || c < 0x20 {
			needsEscape = true
			break
		}
	}
	if !needsEscape {
		return s
	}

	var b strings.Builder
	b.Grow(len(s) + 10)
	for i := 0; i < len(s); i++ {
		c := s[i]
		switch c {
		case '"':
			b.WriteString(`\"`)
		case '\\':
			b.WriteString(`\\`)
		case '\n':
			b.WriteString(`\n`)
		case '\r':
			b.WriteString(`\r`)
		case '\t':
			b.WriteString(`\t`)
		default:
			if c < 0x20 {
				fmt.Fprintf(&b, `\u%04x`, c)
			} else {
				b.WriteByte(c)
			}
		}
	}
	return b.String()
}
