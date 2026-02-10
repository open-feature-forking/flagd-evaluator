package evaluator

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

// FlagEvaluator evaluates feature flags using the flagd-evaluator WASM module.
//
// It is safe for concurrent use from multiple goroutines. All WASM calls are
// serialized with a mutex since WASM linear memory is single-threaded.
type FlagEvaluator struct {
	mu      sync.Mutex
	ctx     context.Context
	runtime wazero.Runtime
	module  api.Module

	// WASM exports
	allocFn        api.Function
	deallocFn      api.Function
	updateStateFn  api.Function
	evalReusableFn api.Function
	evalByIndexFn  api.Function // nil if unavailable

	// Pre-allocated WASM buffers
	flagKeyBufPtr uint32
	contextBufPtr uint32

	// Host-side caches (replaced on UpdateState)
	preEvaluatedCache   map[string]*EvaluationResult
	requiredContextKeys map[string]map[string]bool
	flagIndexCache      map[string]uint32
}

// NewFlagEvaluator creates a new flag evaluator with the given options.
// The WASM module is compiled and instantiated immediately.
// Call Close() when done to release resources.
func NewFlagEvaluator(opts ...Option) (*FlagEvaluator, error) {
	cfg := &evaluatorConfig{}
	for _, opt := range opts {
		opt(cfg)
	}

	ctx := context.Background()

	// Create runtime
	runtimeConfig := wazero.NewRuntimeConfig()
	r := wazero.NewRuntimeWithConfig(ctx, runtimeConfig)

	// Register host functions
	if err := registerHostFunctions(ctx, r); err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to register host functions: %w", err)
	}

	// Compile and instantiate WASM module
	compiled, err := r.CompileModule(ctx, wasmBytes)
	if err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to compile WASM module: %w", err)
	}

	mod, err := r.InstantiateModule(ctx, compiled, wazero.NewModuleConfig().WithName("flagd_evaluator"))
	if err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to instantiate WASM module: %w", err)
	}

	// Look up exports
	allocFn := mod.ExportedFunction("alloc")
	deallocFn := mod.ExportedFunction("dealloc")
	updateStateFn := mod.ExportedFunction("update_state")
	evalReusableFn := mod.ExportedFunction("evaluate_reusable")
	evalByIndexFn := mod.ExportedFunction("evaluate_by_index") // may be nil

	if allocFn == nil || deallocFn == nil || updateStateFn == nil || evalReusableFn == nil {
		r.Close(ctx)
		return nil, fmt.Errorf("WASM module missing required exports")
	}

	// Pre-allocate buffers
	results, err := allocFn.Call(ctx, maxFlagKeySize)
	if err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to allocate flag key buffer: %w", err)
	}
	flagKeyBufPtr := uint32(results[0])

	results, err = allocFn.Call(ctx, maxContextSize)
	if err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to allocate context buffer: %w", err)
	}
	contextBufPtr := uint32(results[0])

	// Set validation mode
	setValidationFn := mod.ExportedFunction("set_validation_mode")
	if setValidationFn != nil {
		mode := uint64(0) // strict
		if cfg.permissiveValidation {
			mode = 1
		}
		_, err = setValidationFn.Call(ctx, mode)
		if err != nil {
			r.Close(ctx)
			return nil, fmt.Errorf("failed to set validation mode: %w", err)
		}
	}

	return &FlagEvaluator{
		ctx:                 ctx,
		runtime:             r,
		module:              mod,
		allocFn:             allocFn,
		deallocFn:           deallocFn,
		updateStateFn:       updateStateFn,
		evalReusableFn:      evalReusableFn,
		evalByIndexFn:       evalByIndexFn,
		flagKeyBufPtr:       flagKeyBufPtr,
		contextBufPtr:       contextBufPtr,
		preEvaluatedCache:   make(map[string]*EvaluationResult),
		requiredContextKeys: make(map[string]map[string]bool),
		flagIndexCache:      make(map[string]uint32),
	}, nil
}

// Close releases all resources associated with the evaluator.
func (e *FlagEvaluator) Close() error {
	e.mu.Lock()
	defer e.mu.Unlock()

	// Free pre-allocated buffers
	if e.deallocFn != nil {
		e.deallocFn.Call(e.ctx, uint64(e.flagKeyBufPtr), maxFlagKeySize)
		e.deallocFn.Call(e.ctx, uint64(e.contextBufPtr), maxContextSize)
	}

	return e.runtime.Close(e.ctx)
}

// UpdateState updates the flag configuration. Returns information about changed
// flags and populates internal caches for pre-evaluated flags, context key
// filtering, and index-based evaluation.
func (e *FlagEvaluator) UpdateState(configJSON string) (*UpdateStateResult, error) {
	e.mu.Lock()
	defer e.mu.Unlock()

	configBytes := []byte(configJSON)

	// Allocate and write config to WASM memory
	configPtr, configLen, err := writeToWasm(e.ctx, e.module, e.allocFn, configBytes)
	if err != nil {
		return nil, fmt.Errorf("failed to write config to WASM: %w", err)
	}
	defer e.deallocFn.Call(e.ctx, uint64(configPtr), uint64(configLen))

	// Call update_state
	results, err := e.updateStateFn.Call(e.ctx, uint64(configPtr), uint64(configLen))
	if err != nil {
		return nil, fmt.Errorf("update_state call failed: %w", err)
	}

	resultPtr, resultLen := unpackPtrLen(results[0])
	resultBytes, err := readFromWasm(e.module, resultPtr, resultLen)
	if err != nil {
		return nil, fmt.Errorf("failed to read update_state result: %w", err)
	}
	defer e.deallocFn.Call(e.ctx, uint64(resultPtr), uint64(resultLen))

	var result UpdateStateResult
	if err := json.Unmarshal(resultBytes, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal update_state result: %w", err)
	}

	// Populate pre-evaluated cache
	if result.PreEvaluated != nil {
		e.preEvaluatedCache = result.PreEvaluated
	} else {
		e.preEvaluatedCache = make(map[string]*EvaluationResult)
	}

	// Populate required context keys cache (convert []string to map[string]bool)
	if result.RequiredContextKeys != nil {
		keyCache := make(map[string]map[string]bool, len(result.RequiredContextKeys))
		for flagKey, keys := range result.RequiredContextKeys {
			keySet := make(map[string]bool, len(keys))
			for _, k := range keys {
				keySet[k] = true
			}
			keyCache[flagKey] = keySet
		}
		e.requiredContextKeys = keyCache
	} else {
		e.requiredContextKeys = make(map[string]map[string]bool)
	}

	// Populate flag index cache
	if result.FlagIndices != nil {
		e.flagIndexCache = result.FlagIndices
	} else {
		e.flagIndexCache = make(map[string]uint32)
	}

	return &result, nil
}
