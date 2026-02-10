package evaluator

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"
	"time"
)

// EvaluateFlag evaluates a flag and returns the full result.
func (e *FlagEvaluator) EvaluateFlag(flagKey string, ctx map[string]interface{}) (*EvaluationResult, error) {
	e.mu.Lock()
	defer e.mu.Unlock()

	return e.evaluateFlagLocked(flagKey, ctx)
}

// EvaluateBool evaluates a boolean flag. Returns defaultValue on error.
func (e *FlagEvaluator) EvaluateBool(flagKey string, ctx map[string]interface{}, defaultValue bool) bool {
	e.mu.Lock()
	defer e.mu.Unlock()

	result, err := e.evaluateFlagLocked(flagKey, ctx)
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
	e.mu.Lock()
	defer e.mu.Unlock()

	result, err := e.evaluateFlagLocked(flagKey, ctx)
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
	e.mu.Lock()
	defer e.mu.Unlock()

	result, err := e.evaluateFlagLocked(flagKey, ctx)
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
	e.mu.Lock()
	defer e.mu.Unlock()

	result, err := e.evaluateFlagLocked(flagKey, ctx)
	if err != nil || result.IsError() || result.Value == nil {
		return defaultValue
	}
	if v, ok := result.Value.(float64); ok {
		return v
	}
	return defaultValue
}

// evaluateFlagLocked is the internal evaluation pipeline. Caller must hold mu.
func (e *FlagEvaluator) evaluateFlagLocked(flagKey string, ctx map[string]interface{}) (result *EvaluationResult, err error) {
	// Recover from WASM panics (__wbindgen_throw)
	defer func() {
		if r := recover(); r != nil {
			result = nil
			err = fmt.Errorf("WASM panic: %v", r)
		}
	}()

	// Fast path: pre-evaluated cache hit (static/disabled flags)
	if cached, ok := e.preEvaluatedCache[flagKey]; ok {
		return cached, nil
	}

	// Determine context serialization strategy
	var contextBytes []byte
	requiredKeys := e.requiredContextKeys[flagKey]
	if requiredKeys != nil && len(ctx) > 0 {
		// Filtered path: only serialize keys the targeting rule references
		contextBytes = serializeFilteredContext(ctx, requiredKeys, flagKey)
	} else if len(ctx) > 0 {
		// Full serialization path
		var jsonErr error
		contextBytes, jsonErr = json.Marshal(ctx)
		if jsonErr != nil {
			return nil, fmt.Errorf("failed to marshal context: %w", jsonErr)
		}
	}

	// Choose evaluation path
	flagIndex, hasIndex := e.flagIndexCache[flagKey]
	if hasIndex && e.evalByIndexFn != nil && requiredKeys != nil {
		// Index-based evaluation (fastest WASM path)
		return e.evaluateByIndex(flagIndex, contextBytes)
	}

	// String-based evaluation with pre-allocated buffer
	return e.evaluateReusable(flagKey, contextBytes)
}

// evaluateByIndex calls the evaluate_by_index WASM export.
func (e *FlagEvaluator) evaluateByIndex(flagIndex uint32, contextBytes []byte) (*EvaluationResult, error) {
	var contextPtr uint32
	var contextLen uint32

	if len(contextBytes) > 0 {
		if err := writeToPreallocBuffer(e.module, e.contextBufPtr, maxContextSize, contextBytes); err != nil {
			return nil, err
		}
		contextPtr = e.contextBufPtr
		contextLen = uint32(len(contextBytes))
	}

	results, err := e.evalByIndexFn.Call(e.ctx, uint64(flagIndex), uint64(contextPtr), uint64(contextLen))
	if err != nil {
		return nil, fmt.Errorf("evaluate_by_index call failed: %w", err)
	}

	return e.readEvalResult(results[0])
}

// evaluateReusable calls the evaluate_reusable WASM export.
func (e *FlagEvaluator) evaluateReusable(flagKey string, contextBytes []byte) (*EvaluationResult, error) {
	flagBytes := []byte(flagKey)
	if err := writeToPreallocBuffer(e.module, e.flagKeyBufPtr, maxFlagKeySize, flagBytes); err != nil {
		return nil, fmt.Errorf("flag key too large: %w", err)
	}

	var contextPtr uint32
	var contextLen uint32

	if len(contextBytes) > 0 {
		if err := writeToPreallocBuffer(e.module, e.contextBufPtr, maxContextSize, contextBytes); err != nil {
			return nil, err
		}
		contextPtr = e.contextBufPtr
		contextLen = uint32(len(contextBytes))
	}

	results, err := e.evalReusableFn.Call(e.ctx,
		uint64(e.flagKeyBufPtr), uint64(len(flagBytes)),
		uint64(contextPtr), uint64(contextLen))
	if err != nil {
		return nil, fmt.Errorf("evaluate_reusable call failed: %w", err)
	}

	return e.readEvalResult(results[0])
}

// readEvalResult reads and unmarshals an evaluation result from a packed u64.
func (e *FlagEvaluator) readEvalResult(packed uint64) (*EvaluationResult, error) {
	resultPtr, resultLen := unpackPtrLen(packed)
	resultBytes, err := readFromWasm(e.module, resultPtr, resultLen)
	if err != nil {
		return nil, fmt.Errorf("failed to read evaluation result: %w", err)
	}
	e.deallocFn.Call(e.ctx, uint64(resultPtr), uint64(resultLen))

	var result EvaluationResult
	if err := json.Unmarshal(resultBytes, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal evaluation result: %w", err)
	}
	return &result, nil
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
