package evaluator

import (
	"context"
	"crypto/rand"
	"fmt"
	"time"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

// registerHostFunctions registers all 9 host functions required by the WASM module.
func registerHostFunctions(ctx context.Context, r wazero.Runtime) error {
	// Module "host" — 1 function
	_, err := r.NewHostModuleBuilder("host").
		NewFunctionBuilder().
		WithFunc(func() int64 {
			return time.Now().Unix()
		}).
		Export("get_current_time_unix_seconds").
		Instantiate(ctx)
	if err != nil {
		return fmt.Errorf("failed to instantiate host module: %w", err)
	}

	// Module "__wbindgen_placeholder__" — 6 functions
	_, err = r.NewHostModuleBuilder("__wbindgen_placeholder__").
		// CRITICAL: random entropy for ahash in boon validation
		NewFunctionBuilder().
		WithFunc(func(ctx context.Context, mod api.Module, _self uint32, bufferPtr uint32) {
			randomBytes := make([]byte, 32)
			_, _ = rand.Read(randomBytes)
			mod.Memory().Write(bufferPtr, randomBytes)
		}).
		Export("__wbg_getRandomValues_1c61fac11405ffdc").
		// LEGACY: Date constructor — returns dummy reference
		NewFunctionBuilder().
		WithFunc(func() int32 {
			return 0
		}).
		Export("__wbg_new_0_23cedd11d9b40c9d").
		// LEGACY: Date.getTime — returns current time millis as f64
		NewFunctionBuilder().
		WithFunc(func(_self int32) float64 {
			return float64(time.Now().UnixMilli())
		}).
		Export("__wbg_getTime_ad1e9878a735af08").
		// ERROR: throws a WASM error — we panic and recover at call boundary
		NewFunctionBuilder().
		WithFunc(func(ctx context.Context, mod api.Module, ptr, length uint32) {
			data, ok := mod.Memory().Read(ptr, length)
			if ok {
				panic(fmt.Sprintf("WASM threw: %s", string(data)))
			}
			panic("WASM threw an error (could not read message)")
		}).
		Export("__wbg___wbindgen_throw_dd24417ed36fc46e").
		// NO-OP: object drop ref
		NewFunctionBuilder().
		WithFunc(func(_idx int32) {}).
		Export("__wbindgen_object_drop_ref").
		// NO-OP: describe
		NewFunctionBuilder().
		WithFunc(func(_idx int32) {}).
		Export("__wbindgen_describe").
		Instantiate(ctx)
	if err != nil {
		return fmt.Errorf("failed to instantiate __wbindgen_placeholder__ module: %w", err)
	}

	// Module "__wbindgen_externref_xform__" — 2 functions
	_, err = r.NewHostModuleBuilder("__wbindgen_externref_xform__").
		NewFunctionBuilder().
		WithFunc(func(_delta int32) int32 {
			return 128
		}).
		Export("__wbindgen_externref_table_grow").
		NewFunctionBuilder().
		WithFunc(func(_idx int32) {}).
		Export("__wbindgen_externref_table_set_null").
		Instantiate(ctx)
	if err != nil {
		return fmt.Errorf("failed to instantiate __wbindgen_externref_xform__ module: %w", err)
	}

	return nil
}
