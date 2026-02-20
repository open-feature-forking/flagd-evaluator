using System.Text;
using Wasmtime;

namespace FlagdEvaluator;

/// <summary>
/// A single WASM module instance with pre-allocated buffers for evaluation.
/// Not thread-safe — each instance is used by one thread at a time via the pool.
/// </summary>
internal sealed class WasmInstance : IDisposable
{
    internal const int MaxFlagKeySize = 256;
    internal const int MaxContextSize = 1024 * 1024; // 1MB

    private readonly Store _store;
    private readonly Instance _instance;
    private readonly Memory _memory;

    // WASM function exports
    private readonly Func<int, int> _alloc;
    private readonly Action<int, int> _dealloc;
    private readonly Func<int, int, long> _updateState;
    private readonly Func<int, int, int, int, long> _evaluateReusable;
    private readonly Func<int, int, int, long>? _evaluateByIndex;

    // Pre-allocated buffers in WASM linear memory
    internal int FlagKeyBufferPtr { get; }
    internal int ContextBufferPtr { get; }

    // Generation stamp — set during UpdateState, checked during evaluation
    internal ulong Generation { get; set; }

    internal WasmInstance(WasmRuntime runtime, bool permissiveValidation)
    {
        var (store, instance) = runtime.CreateInstance();
        _store = store;
        _instance = instance;

        _memory = instance.GetMemory("memory")
            ?? throw new EvaluatorException("WASM module missing 'memory' export");

        _alloc = instance.GetFunction<int, int>("alloc")
            ?? throw new EvaluatorException("WASM module missing 'alloc' export");
        _dealloc = instance.GetAction<int, int>("dealloc")
            ?? throw new EvaluatorException("WASM module missing 'dealloc' export");
        _updateState = instance.GetFunction<int, int, long>("update_state")
            ?? throw new EvaluatorException("WASM module missing 'update_state' export");
        _evaluateReusable = instance.GetFunction<int, int, int, int, long>("evaluate_reusable")
            ?? throw new EvaluatorException("WASM module missing 'evaluate_reusable' export");
        _evaluateByIndex = instance.GetFunction<int, int, int, long>("evaluate_by_index");

        // Pre-allocate buffers
        FlagKeyBufferPtr = _alloc(MaxFlagKeySize);
        ContextBufferPtr = _alloc(MaxContextSize);

        // Set validation mode
        var setValidationFn = instance.GetFunction<int, long>("set_validation_mode");
        if (setValidationFn != null)
        {
            int mode = permissiveValidation ? 1 : 0;
            setValidationFn(mode);
        }
    }

    internal bool HasEvaluateByIndex => _evaluateByIndex != null;

    /// <summary>
    /// Calls update_state on this WASM instance. Allocates memory for the config,
    /// calls the export, reads and copies the result, then deallocates.
    /// </summary>
    internal string CallUpdateState(string configJson)
    {
        var configBytes = Encoding.UTF8.GetBytes(configJson);
        int configPtr = _alloc(configBytes.Length);
        WriteBytes(configPtr, configBytes);

        try
        {
            long packed = _updateState(configPtr, configBytes.Length);
            return ReadAndDeallocResult(packed);
        }
        finally
        {
            _dealloc(configPtr, configBytes.Length);
        }
    }

    /// <summary>
    /// Evaluates a flag using pre-allocated buffers (evaluate_reusable).
    /// </summary>
    internal string CallEvaluateReusable(byte[] flagKeyBytes, byte[] contextBytes)
    {
        if (flagKeyBytes.Length > MaxFlagKeySize)
            throw new EvaluatorException($"Flag key size {flagKeyBytes.Length} exceeds max {MaxFlagKeySize}");

        WriteBytes(FlagKeyBufferPtr, flagKeyBytes);

        int contextPtr = 0;
        int contextLen = 0;
        if (contextBytes.Length > 0)
        {
            if (contextBytes.Length > MaxContextSize)
                throw new EvaluatorException($"Context size {contextBytes.Length} exceeds max {MaxContextSize}");
            WriteBytes(ContextBufferPtr, contextBytes);
            contextPtr = ContextBufferPtr;
            contextLen = contextBytes.Length;
        }

        long packed = _evaluateReusable(FlagKeyBufferPtr, flagKeyBytes.Length, contextPtr, contextLen);
        return ReadAndDeallocResult(packed);
    }

    /// <summary>
    /// Evaluates a flag by index using pre-allocated context buffer.
    /// </summary>
    internal string CallEvaluateByIndex(uint flagIndex, byte[] contextBytes)
    {
        if (_evaluateByIndex == null)
            throw new EvaluatorException("evaluate_by_index export not available");

        int contextPtr = 0;
        int contextLen = 0;
        if (contextBytes.Length > 0)
        {
            if (contextBytes.Length > MaxContextSize)
                throw new EvaluatorException($"Context size {contextBytes.Length} exceeds max {MaxContextSize}");
            WriteBytes(ContextBufferPtr, contextBytes);
            contextPtr = ContextBufferPtr;
            contextLen = contextBytes.Length;
        }

        long packed = _evaluateByIndex((int)flagIndex, contextPtr, contextLen);
        return ReadAndDeallocResult(packed);
    }

    private void WriteBytes(int ptr, byte[] data)
    {
        var span = _memory.GetSpan(ptr, data.Length);
        data.AsSpan().CopyTo(span);
    }

    /// <summary>
    /// Unpacks a result pointer+length, reads the string (copy), then deallocates.
    /// Memory.ReadString creates a managed string copy, so it's safe to dealloc after.
    /// </summary>
    private string ReadAndDeallocResult(long packed)
    {
        int resultPtr = (int)((ulong)packed >> 32);
        int resultLen = (int)((ulong)packed & 0xFFFFFFFFL);

        // ReadString creates a managed copy — safe to dealloc after
        string result = _memory.ReadString(resultPtr, resultLen);
        _dealloc(resultPtr, resultLen);
        return result;
    }

    public void Dispose()
    {
        _dealloc(FlagKeyBufferPtr, MaxFlagKeySize);
        _dealloc(ContextBufferPtr, MaxContextSize);
        _store.Dispose();
    }
}
