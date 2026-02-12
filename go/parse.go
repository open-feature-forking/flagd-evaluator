package evaluator

import (
	"encoding/json"
	"strconv"
	"unsafe"
)

// parseEvalResult is a hand-rolled JSON parser for EvaluationResult.
// It avoids json.Unmarshal's reflection overhead by scanning the known
// field names directly. Falls back to json.Unmarshal for unexpected shapes.
//
// Expected JSON shape from WASM:
//
//	{"value":...,"variant":"...","reason":"...","flagMetadata":{"k":"v","n":1,"b":true}}
//
// flagMetadata values are constrained to string, number, or bool per the flagd spec.
func parseEvalResult(data []byte) (*EvaluationResult, error) {
	var r EvaluationResult

	i := 0
	n := len(data)

	// skip whitespace and opening brace
	for i < n && isWhitespace(data[i]) {
		i++
	}
	if i >= n || data[i] != '{' {
		goto fallback
	}
	i++

	for i < n {
		// skip whitespace
		for i < n && isWhitespace(data[i]) {
			i++
		}
		if i >= n || data[i] == '}' {
			break
		}
		if data[i] == ',' {
			i++
			continue
		}

		// Parse key
		if data[i] != '"' {
			goto fallback
		}
		i++
		keyStart := i
		for i < n && data[i] != '"' {
			if data[i] == '\\' {
				i++
			}
			i++
		}
		if i >= n {
			goto fallback
		}
		key := unsafeBytesToString(data[keyStart:i])
		i++ // skip closing "

		// skip colon
		for i < n && data[i] != ':' {
			i++
		}
		if i >= n {
			goto fallback
		}
		i++

		// skip whitespace before value
		for i < n && isWhitespace(data[i]) {
			i++
		}
		if i >= n {
			goto fallback
		}

		switch key {
		case "variant", "reason", "errorCode", "errorMessage":
			if data[i] != '"' {
				goto fallback
			}
			i++
			valStart := i
			for i < n && data[i] != '"' {
				if data[i] == '\\' {
					i++
				}
				i++
			}
			if i >= n {
				goto fallback
			}
			val := string(data[valStart:i])
			i++

			switch key {
			case "variant":
				r.Variant = val
			case "reason":
				r.Reason = val
			case "errorCode":
				r.ErrorCode = val
			case "errorMessage":
				r.ErrorMessage = val
			}

		case "value":
			var end int
			end, r.Value = parseValue(data, i)
			if end < 0 {
				goto fallback
			}
			i = end

		case "flagMetadata":
			if data[i] != '{' {
				goto fallback
			}
			meta, end := parseMetadata(data, i)
			if end < 0 {
				goto fallback
			}
			r.FlagMetadata = meta
			i = end

		default:
			// Skip unknown field value
			end := skipValue(data, i)
			if end < 0 {
				goto fallback
			}
			i = end
		}
	}
	return &r, nil

fallback:
	var rf EvaluationResult
	if err := json.Unmarshal(data, &rf); err != nil {
		return nil, err
	}
	return &rf, nil
}

// parseValue parses a JSON value starting at data[i].
// Returns (new index, parsed value). Returns (-1, nil) on error.
func parseValue(data []byte, i int) (int, interface{}) {
	n := len(data)
	if i >= n {
		return -1, nil
	}

	switch data[i] {
	case 't': // true
		if i+4 <= n {
			return i + 4, true
		}
		return -1, nil
	case 'f': // false
		if i+5 <= n {
			return i + 5, false
		}
		return -1, nil
	case 'n': // null
		if i+4 <= n {
			return i + 4, nil
		}
		return -1, nil
	case '"': // string
		i++
		strStart := i
		for i < n && data[i] != '"' {
			if data[i] == '\\' {
				i++
			}
			i++
		}
		if i >= n {
			return -1, nil
		}
		val := string(data[strStart:i])
		return i + 1, val
	default:
		// number or complex type — find extent, unmarshal
		valStart := i
		depth := 0
		for i < n {
			switch data[i] {
			case '{', '[':
				depth++
			case '}', ']':
				if depth == 0 {
					goto numEnd
				}
				depth--
			case ',':
				if depth == 0 {
					goto numEnd
				}
			case '"':
				i++
				for i < n && data[i] != '"' {
					if data[i] == '\\' {
						i++
					}
					i++
				}
			}
			i++
		}
	numEnd:
		valBytes := data[valStart:i]
		// Fast path: try parsing as number directly
		if f, err := strconv.ParseFloat(unsafeBytesToString(valBytes), 64); err == nil {
			return i, f
		}
		// Complex type fallback
		var v interface{}
		if err := json.Unmarshal(valBytes, &v); err != nil {
			return -1, nil
		}
		return i, v
	}
}

// parseMetadata parses a flat JSON object with string/number/bool values.
// Starts at data[i] which must be '{'.
// Returns (map, new index). Returns (nil, -1) on error.
func parseMetadata(data []byte, i int) (map[string]interface{}, int) {
	n := len(data)
	i++ // skip '{'
	meta := make(map[string]interface{})

	for i < n {
		for i < n && isWhitespace(data[i]) {
			i++
		}
		if i >= n {
			return nil, -1
		}
		if data[i] == '}' {
			return meta, i + 1
		}
		if data[i] == ',' {
			i++
			continue
		}

		// Parse key
		if data[i] != '"' {
			return nil, -1
		}
		i++
		keyStart := i
		for i < n && data[i] != '"' {
			if data[i] == '\\' {
				i++
			}
			i++
		}
		if i >= n {
			return nil, -1
		}
		key := string(data[keyStart:i])
		i++ // skip closing "

		// skip colon and whitespace
		for i < n && (isWhitespace(data[i]) || data[i] == ':') {
			i++
		}
		if i >= n {
			return nil, -1
		}

		// Parse value (string, number, or bool only)
		switch data[i] {
		case '"': // string
			i++
			valStart := i
			for i < n && data[i] != '"' {
				if data[i] == '\\' {
					i++
				}
				i++
			}
			if i >= n {
				return nil, -1
			}
			meta[key] = string(data[valStart:i])
			i++

		case 't': // true
			meta[key] = true
			i += 4

		case 'f': // false
			meta[key] = false
			i += 5

		default: // number
			numStart := i
			for i < n && data[i] != ',' && data[i] != '}' && !isWhitespace(data[i]) {
				i++
			}
			f, err := strconv.ParseFloat(string(data[numStart:i]), 64)
			if err != nil {
				return nil, -1
			}
			meta[key] = f
		}
	}

	return nil, -1
}

// skipValue skips over a JSON value starting at data[i].
// Returns the new index, or -1 on error.
func skipValue(data []byte, i int) int {
	n := len(data)
	if i >= n {
		return -1
	}

	switch data[i] {
	case '"':
		i++
		for i < n && data[i] != '"' {
			if data[i] == '\\' {
				i++
			}
			i++
		}
		if i >= n {
			return -1
		}
		return i + 1

	case 't':
		return i + 4
	case 'f':
		return i + 5
	case 'n':
		return i + 4

	case '{', '[':
		open := data[i]
		close := byte('}')
		if open == '[' {
			close = ']'
		}
		depth := 1
		i++
		for i < n && depth > 0 {
			switch data[i] {
			case open:
				depth++
			case close:
				depth--
			case '"':
				i++
				for i < n && data[i] != '"' {
					if data[i] == '\\' {
						i++
					}
					i++
				}
			}
			i++
		}
		return i

	default: // number
		for i < n && data[i] != ',' && data[i] != '}' && data[i] != ']' && !isWhitespace(data[i]) {
			i++
		}
		return i
	}
}

func isWhitespace(b byte) bool {
	return b == ' ' || b == '\t' || b == '\n' || b == '\r'
}

func unsafeBytesToString(b []byte) string {
	return *(*string)(unsafe.Pointer(&b))
}
