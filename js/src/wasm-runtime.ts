import { readFileSync } from "node:fs";
import { webcrypto } from "node:crypto";

/** WASM exports from the flagd-evaluator module. */
export interface WasmExports {
  memory: WebAssembly.Memory;
  alloc: (len: number) => number;
  dealloc: (ptr: number, len: number) => void;
  update_state: (configPtr: number, configLen: number) => bigint;
  evaluate_reusable: (
    flagKeyPtr: number,
    flagKeyLen: number,
    contextPtr: number,
    contextLen: number,
  ) => bigint;
  evaluate_by_index?: (
    flagIndex: number,
    contextPtr: number,
    contextLen: number,
  ) => bigint;
  set_validation_mode: (mode: number) => bigint;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Unpack a packed u64 into (ptr, len). Upper 32 = ptr, lower 32 = len. */
export function unpackPtrLen(packed: bigint): [number, number] {
  const ptr = Number(packed >> 32n);
  const len = Number(packed & 0xffffffffn);
  return [ptr, len];
}

/** Read bytes from WASM memory, copying before any further WASM calls. */
export function readString(
  memory: WebAssembly.Memory,
  ptr: number,
  len: number,
): string {
  // CRITICAL: slice() copies the data — buffer may detach after WASM calls
  const bytes = new Uint8Array(memory.buffer, ptr, len).slice();
  return decoder.decode(bytes);
}

/** Write a UTF-8 string into a pre-allocated WASM buffer. Returns byte length. */
export function writeToBuffer(
  memory: WebAssembly.Memory,
  bufferPtr: number,
  text: string,
): number {
  const bytes = encoder.encode(text);
  new Uint8Array(memory.buffer).set(bytes, bufferPtr);
  return bytes.byteLength;
}

/** Build the import object for WASM instantiation using prefix-based matching. */
function buildImports(
  moduleImports: WebAssembly.ModuleImportDescriptor[],
  memoryRef: { memory: WebAssembly.Memory | null },
): WebAssembly.Imports {
  const imports: WebAssembly.Imports = {};

  // Collect required modules and function names
  const required = new Map<string, Set<string>>();
  for (const imp of moduleImports) {
    if (imp.kind !== "function") continue;
    if (!required.has(imp.module)) required.set(imp.module, new Set());
    required.get(imp.module)!.add(imp.name);
  }

  // Host functions by module and prefix
  const hostFunctions: Record<
    string,
    Record<string, (...args: number[]) => number | bigint | void>
  > = {
    host: {
      get_current_time_unix_seconds: () =>
        BigInt(Math.floor(Date.now() / 1000)),
    },
    __wbindgen_placeholder__: {
      __wbg_getRandomValues_: (
        _self: number,
        bufferPtr: number,
      ) => {
        const randomBytes = new Uint8Array(32);
        (webcrypto as unknown as Crypto).getRandomValues(randomBytes);
        new Uint8Array(memoryRef.memory!.buffer).set(randomBytes, bufferPtr);
      },
      __wbg_new_0_: () => 0,
      __wbg_getTime_: () => Date.now(),
      __wbg___wbindgen_throw: (ptr: number, len: number) => {
        const msg = readString(memoryRef.memory!, ptr, len);
        throw new Error(`WASM threw: ${msg}`);
      },
      __wbindgen_object_drop_ref: () => {},
      __wbindgen_describe: () => {},
    },
    __wbindgen_externref_xform__: {
      __wbindgen_externref_table_grow: () => 128,
      __wbindgen_externref_table_set_null: () => {},
    },
  };

  // Match imports by prefix
  for (const [moduleName, funcNames] of required) {
    const moduleImportObj: Record<
      string,
      (...args: number[]) => number | bigint | void
    > = {};
    const handlers = hostFunctions[moduleName];

    if (!handlers) {
      // Unknown module — provide no-op stubs
      for (const name of funcNames) {
        moduleImportObj[name] = () => {};
      }
      imports[moduleName] = moduleImportObj;
      continue;
    }

    for (const name of funcNames) {
      // Exact match first, then prefix match
      if (handlers[name]) {
        moduleImportObj[name] = handlers[name];
      } else {
        const prefix = Object.keys(handlers).find((p) => name.startsWith(p));
        if (prefix) {
          moduleImportObj[name] = handlers[prefix];
        } else {
          moduleImportObj[name] = () => {};
        }
      }
    }
    imports[moduleName] = moduleImportObj;
  }

  return imports;
}

/** Load and instantiate the WASM module. */
export async function loadWasm(wasmPath: string): Promise<WasmExports> {
  const wasmBytes = readFileSync(wasmPath);
  const module = await WebAssembly.compile(wasmBytes);
  const moduleImports = WebAssembly.Module.imports(module);

  // Use a ref object so host functions can access memory after instantiation
  const memoryRef: { memory: WebAssembly.Memory | null } = { memory: null };
  const imports = buildImports(moduleImports, memoryRef);

  const instance = await WebAssembly.instantiate(module, imports);
  const exports = instance.exports as unknown as WasmExports;
  memoryRef.memory = exports.memory;

  return exports;
}
