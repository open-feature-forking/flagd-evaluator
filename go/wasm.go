package evaluator

import (
	"context"
	_ "embed"
	"fmt"

	"github.com/tetratelabs/wazero/api"
)

//go:embed flagd_evaluator.wasm
var wasmBytes []byte

// Pre-allocated buffer sizes matching Java implementation
const (
	maxFlagKeySize = 256
	maxContextSize = 1024 * 1024 // 1MB
)

// unpackPtrLen unpacks a u64 return value into pointer (upper 32) and length (lower 32).
func unpackPtrLen(packed uint64) (ptr, length uint32) {
	ptr = uint32(packed >> 32)
	length = uint32(packed & 0xFFFFFFFF)
	return
}

// writeToWasm allocates WASM memory and writes data to it. Returns pointer and length.
// The caller must dealloc the returned pointer.
func writeToWasm(ctx context.Context, mod api.Module, allocFn api.Function, data []byte) (uint32, uint32, error) {
	dataLen := uint32(len(data))
	results, err := allocFn.Call(ctx, uint64(dataLen))
	if err != nil {
		return 0, 0, fmt.Errorf("alloc failed: %w", err)
	}
	ptr := uint32(results[0])

	if !mod.Memory().Write(ptr, data) {
		return 0, 0, fmt.Errorf("memory write failed at ptr=%d len=%d", ptr, dataLen)
	}
	return ptr, dataLen, nil
}

// readFromWasm reads bytes from WASM linear memory.
// Returns a copy since wazero's Memory.Read returns a view that may be
// invalidated by subsequent WASM calls (e.g., dealloc).
func readFromWasm(mod api.Module, ptr, length uint32) ([]byte, error) {
	view, ok := mod.Memory().Read(ptr, length)
	if !ok {
		return nil, fmt.Errorf("memory read failed at ptr=%d len=%d", ptr, length)
	}
	data := make([]byte, length)
	copy(data, view)
	return data, nil
}

// writeToPreallocBuffer writes data to a pre-allocated buffer with bounds checking.
func writeToPreallocBuffer(mod api.Module, bufPtr, bufSize uint32, data []byte) error {
	if uint32(len(data)) > bufSize {
		return fmt.Errorf("data size %d exceeds buffer size %d", len(data), bufSize)
	}
	if !mod.Memory().Write(bufPtr, data) {
		return fmt.Errorf("memory write failed at ptr=%d len=%d", bufPtr, len(data))
	}
	return nil
}
