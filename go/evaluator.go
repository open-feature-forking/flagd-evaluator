package evaluator

import (
	"context"
	"encoding/json"
	"fmt"
	"runtime"
	"sync"
	"sync/atomic"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

// wasmInstance holds per-instance WASM state. Each instance has its own
// linear memory and can evaluate independently.
type wasmInstance struct {
	module         api.Module
	allocFn        api.Function
	deallocFn      api.Function
	updateStateFn  api.Function
	evalReusableFn api.Function
	evalByIndexFn  api.Function // nil if unavailable
	flagKeyBufPtr  uint32
	contextBufPtr  uint32
	generation     uint64 // set during UpdateState
}

// cacheSnapshot holds all host-side caches. Replaced atomically on UpdateState.
type cacheSnapshot struct {
	generation     uint64
	preEvaluated   map[string]*EvaluationResult
	requiredCtxKey map[string]map[string]bool
	flagIndex      map[string]uint32
}

// FlagEvaluator evaluates feature flags using a pool of flagd-evaluator WASM
// instances. It is safe for concurrent use from multiple goroutines.
//
// Pre-evaluated (static/disabled) flags are served lock-free via atomic cache.
// Targeting flags evaluate in parallel up to the pool size.
type FlagEvaluator struct {
	ctx      context.Context
	rt       wazero.Runtime
	compiled wazero.CompiledModule

	// Pool of WASM instances (buffered channel)
	pool     chan *wasmInstance
	poolSize int

	// Host-side caches — atomically swapped on UpdateState
	cache atomic.Pointer[cacheSnapshot]

	// Serializes UpdateState calls
	updateMu sync.Mutex

	// Generation counter — incremented on each UpdateState
	generation atomic.Uint64

	// Config retained for creating new instances
	permissiveValidation bool
}

// NewFlagEvaluator creates a new flag evaluator with the given options.
// The WASM module is compiled once, then instantiated poolSize times.
// Call Close() when done to release resources.
func NewFlagEvaluator(opts ...Option) (*FlagEvaluator, error) {
	cfg := &evaluatorConfig{}
	for _, opt := range opts {
		opt(cfg)
	}

	poolSize := cfg.poolSize
	if poolSize <= 0 {
		poolSize = runtime.NumCPU()
	}

	ctx := context.Background()

	// Create runtime
	r := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig())

	// Register host functions (shared across all instances)
	if err := registerHostFunctions(ctx, r); err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to register host functions: %w", err)
	}

	// Compile WASM module once
	compiled, err := r.CompileModule(ctx, wasmBytes)
	if err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to compile WASM module: %w", err)
	}

	e := &FlagEvaluator{
		ctx:                  ctx,
		rt:                   r,
		compiled:             compiled,
		pool:                 make(chan *wasmInstance, poolSize),
		poolSize:             poolSize,
		permissiveValidation: cfg.permissiveValidation,
	}

	// Store empty cache
	e.cache.Store(&cacheSnapshot{
		preEvaluated:   make(map[string]*EvaluationResult),
		requiredCtxKey: make(map[string]map[string]bool),
		flagIndex:      make(map[string]uint32),
	})

	// Create pool of instances
	for i := 0; i < poolSize; i++ {
		inst, err := e.newInstance(i)
		if err != nil {
			e.Close()
			return nil, fmt.Errorf("failed to create WASM instance %d: %w", i, err)
		}
		e.pool <- inst
	}

	return e, nil
}

// newInstance creates a single WASM module instance with pre-allocated buffers.
func (e *FlagEvaluator) newInstance(id int) (*wasmInstance, error) {
	name := fmt.Sprintf("flagd_evaluator_%d", id)
	mod, err := e.rt.InstantiateModule(e.ctx, e.compiled,
		wazero.NewModuleConfig().WithName(name))
	if err != nil {
		return nil, fmt.Errorf("failed to instantiate module %q: %w", name, err)
	}

	allocFn := mod.ExportedFunction("alloc")
	deallocFn := mod.ExportedFunction("dealloc")
	updateStateFn := mod.ExportedFunction("update_state")
	evalReusableFn := mod.ExportedFunction("evaluate_reusable")
	evalByIndexFn := mod.ExportedFunction("evaluate_by_index") // may be nil

	if allocFn == nil || deallocFn == nil || updateStateFn == nil || evalReusableFn == nil {
		mod.Close(e.ctx)
		return nil, fmt.Errorf("WASM module missing required exports")
	}

	// Pre-allocate buffers
	results, err := allocFn.Call(e.ctx, maxFlagKeySize)
	if err != nil {
		mod.Close(e.ctx)
		return nil, fmt.Errorf("failed to allocate flag key buffer: %w", err)
	}
	flagKeyBufPtr := uint32(results[0])

	results, err = allocFn.Call(e.ctx, maxContextSize)
	if err != nil {
		mod.Close(e.ctx)
		return nil, fmt.Errorf("failed to allocate context buffer: %w", err)
	}
	contextBufPtr := uint32(results[0])

	// Set validation mode
	setValidationFn := mod.ExportedFunction("set_validation_mode")
	if setValidationFn != nil {
		mode := uint64(0) // strict
		if e.permissiveValidation {
			mode = 1
		}
		if _, err := setValidationFn.Call(e.ctx, mode); err != nil {
			mod.Close(e.ctx)
			return nil, fmt.Errorf("failed to set validation mode: %w", err)
		}
	}

	return &wasmInstance{
		module:         mod,
		allocFn:        allocFn,
		deallocFn:      deallocFn,
		updateStateFn:  updateStateFn,
		evalReusableFn: evalReusableFn,
		evalByIndexFn:  evalByIndexFn,
		flagKeyBufPtr:  flagKeyBufPtr,
		contextBufPtr:  contextBufPtr,
	}, nil
}

// Close releases all resources associated with the evaluator.
func (e *FlagEvaluator) Close() error {
	// Drain and close all instances
	for i := 0; i < e.poolSize; i++ {
		select {
		case inst := <-e.pool:
			inst.deallocFn.Call(e.ctx, uint64(inst.flagKeyBufPtr), maxFlagKeySize)
			inst.deallocFn.Call(e.ctx, uint64(inst.contextBufPtr), maxContextSize)
			inst.module.Close(e.ctx)
		default:
			// Instance is in use; skip (runtime.Close will clean up)
		}
	}
	return e.rt.Close(e.ctx)
}

// UpdateState updates the flag configuration across all WASM instances.
// Returns information about changed flags and populates internal caches.
func (e *FlagEvaluator) UpdateState(configJSON string) (*UpdateStateResult, error) {
	e.updateMu.Lock()
	defer e.updateMu.Unlock()

	configBytes := []byte(configJSON)

	// Drain all instances from pool (blocks until all are returned)
	instances := make([]*wasmInstance, e.poolSize)
	for i := 0; i < e.poolSize; i++ {
		instances[i] = <-e.pool
	}

	// Update first instance and capture result
	result, err := updateInstance(e.ctx, instances[0], configBytes)
	if err != nil {
		// Return all instances before failing
		for _, inst := range instances {
			e.pool <- inst
		}
		return nil, err
	}

	// Update remaining instances in parallel
	if len(instances) > 1 {
		var wg sync.WaitGroup
		wg.Add(len(instances) - 1)
		for _, inst := range instances[1:] {
			go func(inst *wasmInstance) {
				defer wg.Done()
				updateInstance(e.ctx, inst, configBytes)
			}(inst)
		}
		wg.Wait()
	}

	// Increment generation and stamp on cache + all instances
	gen := e.generation.Add(1)

	snap := buildCacheSnapshot(result)
	snap.generation = gen

	for _, inst := range instances {
		inst.generation = gen
	}

	// Atomically swap caches, then return instances
	e.cache.Store(snap)
	for _, inst := range instances {
		e.pool <- inst
	}

	return result, nil
}

// updateInstance calls update_state on a single WASM instance.
func updateInstance(ctx context.Context, inst *wasmInstance, configBytes []byte) (*UpdateStateResult, error) {
	configPtr, configLen, err := writeToWasm(ctx, inst.module, inst.allocFn, configBytes)
	if err != nil {
		return nil, fmt.Errorf("failed to write config to WASM: %w", err)
	}
	defer inst.deallocFn.Call(ctx, uint64(configPtr), uint64(configLen))

	results, err := inst.updateStateFn.Call(ctx, uint64(configPtr), uint64(configLen))
	if err != nil {
		return nil, fmt.Errorf("update_state call failed: %w", err)
	}

	resultPtr, resultLen := unpackPtrLen(results[0])
	resultBytes, err := readFromWasm(inst.module, resultPtr, resultLen)
	if err != nil {
		return nil, fmt.Errorf("failed to read update_state result: %w", err)
	}
	defer inst.deallocFn.Call(ctx, uint64(resultPtr), uint64(resultLen))

	var result UpdateStateResult
	if err := json.Unmarshal(resultBytes, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal update_state result: %w", err)
	}
	return &result, nil
}

// buildCacheSnapshot constructs a cacheSnapshot from an UpdateStateResult.
func buildCacheSnapshot(result *UpdateStateResult) *cacheSnapshot {
	snap := &cacheSnapshot{
		preEvaluated:   make(map[string]*EvaluationResult),
		requiredCtxKey: make(map[string]map[string]bool),
		flagIndex:      make(map[string]uint32),
	}

	if result.PreEvaluated != nil {
		snap.preEvaluated = result.PreEvaluated
	}

	if result.RequiredContextKeys != nil {
		keyCache := make(map[string]map[string]bool, len(result.RequiredContextKeys))
		for flagKey, keys := range result.RequiredContextKeys {
			keySet := make(map[string]bool, len(keys))
			for _, k := range keys {
				keySet[k] = true
			}
			keyCache[flagKey] = keySet
		}
		snap.requiredCtxKey = keyCache
	}

	if result.FlagIndices != nil {
		snap.flagIndex = result.FlagIndices
	}

	return snap
}
