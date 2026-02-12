package evaluator

import (
	"encoding/json"
	"testing"
)

// Sample WASM result JSONs
var (
	boolResult   = []byte(`{"value":true,"variant":"on","reason":"TARGETING_MATCH"}`)
	stringResult = []byte(`{"value":"world","variant":"hello","reason":"TARGETING_MATCH"}`)
	metaResult   = []byte(`{"value":true,"variant":"on","reason":"TARGETING_MATCH","flagMetadata":{"key1":"val1","version":1,"enabled":true}}`)
	errorResult  = []byte(`{"reason":"ERROR","errorCode":"FLAG_NOT_FOUND","errorMessage":"flag not found"}`)
	numberResult = []byte(`{"value":3.14,"variant":"pi","reason":"TARGETING_MATCH"}`)
)

func BenchmarkParse_StdLib(b *testing.B) {
	for i := 0; i < b.N; i++ {
		var r EvaluationResult
		json.Unmarshal(boolResult, &r)
	}
}

func BenchmarkParse_HandRolled(b *testing.B) {
	for i := 0; i < b.N; i++ {
		parseEvalResult(boolResult)
	}
}

func BenchmarkParse_StdLib_String(b *testing.B) {
	for i := 0; i < b.N; i++ {
		var r EvaluationResult
		json.Unmarshal(stringResult, &r)
	}
}

func BenchmarkParse_HandRolled_String(b *testing.B) {
	for i := 0; i < b.N; i++ {
		parseEvalResult(stringResult)
	}
}

func BenchmarkParse_StdLib_Meta(b *testing.B) {
	for i := 0; i < b.N; i++ {
		var r EvaluationResult
		json.Unmarshal(metaResult, &r)
	}
}

func BenchmarkParse_HandRolled_Meta(b *testing.B) {
	for i := 0; i < b.N; i++ {
		parseEvalResult(metaResult)
	}
}

func BenchmarkParse_StdLib_Error(b *testing.B) {
	for i := 0; i < b.N; i++ {
		var r EvaluationResult
		json.Unmarshal(errorResult, &r)
	}
}

func BenchmarkParse_HandRolled_Error(b *testing.B) {
	for i := 0; i < b.N; i++ {
		parseEvalResult(errorResult)
	}
}

func BenchmarkParse_StdLib_Number(b *testing.B) {
	for i := 0; i < b.N; i++ {
		var r EvaluationResult
		json.Unmarshal(numberResult, &r)
	}
}

func BenchmarkParse_HandRolled_Number(b *testing.B) {
	for i := 0; i < b.N; i++ {
		parseEvalResult(numberResult)
	}
}

// Correctness test
func TestParseEvalResult(t *testing.T) {
	tests := []struct {
		name string
		data []byte
	}{
		{"bool", boolResult},
		{"string", stringResult},
		{"meta", metaResult},
		{"error", errorResult},
		{"number", numberResult},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var want EvaluationResult
			json.Unmarshal(tt.data, &want)

			got, err := parseEvalResult(tt.data)
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}

			wantJSON, _ := json.Marshal(want)
			gotJSON, _ := json.Marshal(got)
			if string(wantJSON) != string(gotJSON) {
				t.Errorf("mismatch:\n  want: %s\n  got:  %s", wantJSON, gotJSON)
			}
		})
	}
}

func TestParseEvalResult_MetadataTypes(t *testing.T) {
	data := []byte(`{"value":true,"variant":"on","reason":"STATIC","flagMetadata":{"str":"hello","num":42,"float":3.14,"bool_t":true,"bool_f":false}}`)

	got, err := parseEvalResult(data)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if got.FlagMetadata == nil {
		t.Fatal("expected flagMetadata to be non-nil")
	}
	if got.FlagMetadata["str"] != "hello" {
		t.Errorf("str: got %v, want hello", got.FlagMetadata["str"])
	}
	if got.FlagMetadata["num"] != float64(42) {
		t.Errorf("num: got %v, want 42", got.FlagMetadata["num"])
	}
	if got.FlagMetadata["float"] != 3.14 {
		t.Errorf("float: got %v, want 3.14", got.FlagMetadata["float"])
	}
	if got.FlagMetadata["bool_t"] != true {
		t.Errorf("bool_t: got %v, want true", got.FlagMetadata["bool_t"])
	}
	if got.FlagMetadata["bool_f"] != false {
		t.Errorf("bool_f: got %v, want false", got.FlagMetadata["bool_f"])
	}
}
