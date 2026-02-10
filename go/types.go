package evaluator

// EvaluationResult contains the result of a flag evaluation.
type EvaluationResult struct {
	Value        interface{}            `json:"value"`
	Variant      string                 `json:"variant,omitempty"`
	Reason       string                 `json:"reason"`
	ErrorCode    string                 `json:"errorCode,omitempty"`
	ErrorMessage string                 `json:"errorMessage,omitempty"`
	FlagMetadata map[string]interface{} `json:"flagMetadata,omitempty"`
}

// IsError returns true if the evaluation resulted in an error.
func (r *EvaluationResult) IsError() bool {
	return r.ErrorCode != ""
}

// UpdateStateResult contains the result of updating flag state.
type UpdateStateResult struct {
	Success             bool                         `json:"success"`
	Error               string                       `json:"error,omitempty"`
	ChangedFlags        []string                     `json:"changedFlags,omitempty"`
	PreEvaluated        map[string]*EvaluationResult `json:"preEvaluated,omitempty"`
	RequiredContextKeys map[string][]string          `json:"requiredContextKeys,omitempty"`
	FlagIndices         map[string]uint32            `json:"flagIndices,omitempty"`
}

// Option configures a FlagEvaluator.
type Option func(*evaluatorConfig)

type evaluatorConfig struct {
	permissiveValidation bool
	compilationCache     interface{} // wazero.CompilationCache
}

// WithPermissiveValidation configures the evaluator to accept invalid flag
// configurations with warnings instead of rejecting them.
func WithPermissiveValidation() Option {
	return func(c *evaluatorConfig) {
		c.permissiveValidation = true
	}
}

// Evaluation reasons
const (
	ReasonStatic         = "STATIC"
	ReasonDefault        = "DEFAULT"
	ReasonTargetingMatch = "TARGETING_MATCH"
	ReasonDisabled       = "DISABLED"
	ReasonError          = "ERROR"
	ReasonFlagNotFound   = "FLAG_NOT_FOUND"
)

// Error codes
const (
	ErrorFlagNotFound = "FLAG_NOT_FOUND"
	ErrorParseError   = "PARSE_ERROR"
	ErrorTypeMismatch = "TYPE_MISMATCH"
	ErrorGeneral      = "GENERAL"
)
