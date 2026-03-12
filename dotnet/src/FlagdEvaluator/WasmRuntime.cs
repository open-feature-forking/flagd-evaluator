using System.Reflection;
using System.Security.Cryptography;
using Wasmtime;

namespace FlagdEvaluator;

/// <summary>
/// Manages the shared Wasmtime Engine, compiled Module, and Linker with host functions.
/// Thread-safe: Engine and Module are compiled once; Linker definitions are store-independent.
/// </summary>
internal sealed class WasmRuntime : IDisposable
{
    private readonly Engine _engine;
    private readonly Wasmtime.Module _module;
    private readonly Linker _linker;

    internal Engine Engine => _engine;

    internal WasmRuntime()
    {
        _engine = new Engine();

        var assembly = Assembly.GetExecutingAssembly();
        using var stream = assembly.GetManifestResourceStream("flagd_evaluator.wasm")
            ?? throw new EvaluatorException("Embedded WASM resource 'flagd_evaluator.wasm' not found");

        using var ms = new MemoryStream();
        stream.CopyTo(ms);
        _module = Wasmtime.Module.FromBytes(_engine, "flagd_evaluator", ms.ToArray());

        _linker = new Linker(_engine);
        RegisterHostFunctions();
    }

    /// <summary>
    /// Creates a new WASM instance (Store + Instance pair).
    /// Each instance has its own linear memory and can evaluate independently.
    /// </summary>
    internal (Store Store, Instance Instance) CreateInstance()
    {
        var store = new Store(_engine);
        var instance = _linker.Instantiate(store, _module);
        return (store, instance);
    }

    private void RegisterHostFunctions()
    {
        // Build a lookup of imports by module+prefix for prefix-based matching
        var imports = new List<(string Module, string Name)>();
        foreach (var import in _module.Imports)
        {
            imports.Add((import.ModuleName, import.Name));
        }

        // Module "host" — required: provides wall-clock time for $flagd.timestamp enrichment
        var timeFnName = FindImport(imports, "host", "get_current_time_unix_seconds")
            ?? throw new EvaluatorException("WASM module missing required import: host::get_current_time_unix_seconds");
        _linker.DefineFunction("host", timeFnName,
            () => DateTimeOffset.UtcNow.ToUnixTimeSeconds());

        // Module "__wbindgen_placeholder__" — optional, only present in wasm-bindgen builds.
        // Clean builds (getrandom custom backend) have no wasm-bindgen imports at all.

        var randomFnName = FindImport(imports, "__wbindgen_placeholder__", "__wbg_getRandomValues_");
        if (randomFnName != null)
            _linker.DefineFunction("__wbindgen_placeholder__", randomFnName,
                (Caller caller, int _self, int bufferPtr) =>
                {
                    var memory = caller.GetMemory("memory")!;
                    Span<byte> randomBytes = stackalloc byte[32];
                    RandomNumberGenerator.Fill(randomBytes);
                    var span = memory.GetSpan(bufferPtr, 32);
                    randomBytes.CopyTo(span);
                });

        var newDateFnName = FindImport(imports, "__wbindgen_placeholder__", "__wbg_new_0_");
        if (newDateFnName != null)
            _linker.DefineFunction("__wbindgen_placeholder__", newDateFnName, () => 0);

        var getTimeFnName = FindImport(imports, "__wbindgen_placeholder__", "__wbg_getTime_");
        if (getTimeFnName != null)
            _linker.DefineFunction("__wbindgen_placeholder__", getTimeFnName,
                (int _self) => (double)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

        var throwFnName = FindImport(imports, "__wbindgen_placeholder__", "__wbg___wbindgen_throw");
        if (throwFnName != null)
            _linker.DefineFunction("__wbindgen_placeholder__", throwFnName,
                (Caller caller, int ptr, int len) =>
                {
                    var memory = caller.GetMemory("memory")!;
                    var message = memory.ReadString(ptr, len);
                    throw new EvaluatorException($"WASM threw: {message}");
                });

        var dropRefFnName = FindImport(imports, "__wbindgen_placeholder__", "__wbindgen_object_drop_ref");
        if (dropRefFnName != null)
            _linker.DefineFunction("__wbindgen_placeholder__", dropRefFnName, (int _idx) => { });

        var describeFnName = FindImport(imports, "__wbindgen_placeholder__", "__wbindgen_describe");
        if (describeFnName != null)
            _linker.DefineFunction("__wbindgen_placeholder__", describeFnName, (int _idx) => { });

        // Module "__wbindgen_externref_xform__" — optional, only in wasm-bindgen builds
        var tableGrowFnName = FindImport(imports, "__wbindgen_externref_xform__", "__wbindgen_externref_table_grow");
        if (tableGrowFnName != null)
            _linker.DefineFunction("__wbindgen_externref_xform__", tableGrowFnName, (int _delta) => 128);

        var tableSetNullFnName = FindImport(imports, "__wbindgen_externref_xform__", "__wbindgen_externref_table_set_null");
        if (tableSetNullFnName != null)
            _linker.DefineFunction("__wbindgen_externref_xform__", tableSetNullFnName, (int _idx) => { });
    }

    /// <summary>
    /// Finds an import name by module and prefix. Returns null if not found.
    /// This survives wasm-bindgen hash suffix changes across WASM rebuilds.
    /// </summary>
    private static string? FindImport(List<(string Module, string Name)> imports, string module, string prefix)
    {
        foreach (var (mod, name) in imports)
        {
            if (mod == module && name.StartsWith(prefix, StringComparison.Ordinal))
                return name;
        }
        return null;
    }

    public void Dispose()
    {
        _linker.Dispose();
        _module.Dispose();
        _engine.Dispose();
    }
}
