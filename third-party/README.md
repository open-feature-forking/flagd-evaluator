# Third-Party Dependencies

This directory contains patched versions of third-party dependencies that require modifications to work correctly in pure WASM environments like Chicory.

## chrono

**Version:** 0.4.42  
**Original Source:** https://github.com/chronotope/chrono  
**License:** MIT OR Apache-2.0

### Modification

The only change from upstream chrono is in `Cargo.toml`:

```diff
- default = ["clock", "std", "oldtime", "wasmbind"]
+ default = ["clock", "std", "oldtime"]
```

### Reason

The `wasmbind` feature enables `wasm-bindgen` and `js-sys` dependencies for browser-based WASM environments. These create import statements in the compiled WASM module that reference JavaScript host functions (like `__wbindgen_placeholder__.__wbindgen_object_drop_ref`).

Pure WASM runtimes like Chicory (Java) don't provide these JavaScript host functions, causing the WASM module to fail to load with errors like:

```
UnlinkableException: unknown import, could not find host function for import number: 0 named __wbindgen_placeholder__.__wbindgen_object_drop_ref
```

By removing `wasmbind` from the default features, the compiled WASM module has no external imports and can load successfully in any WASM runtime.

### Updating

To update chrono:

1. Download the new version from https://github.com/chronotope/chrono/releases
2. Copy `Cargo.toml`, `LICENSE.txt`, and `src/` to this directory
3. Remove `wasmbind` from the `default` features in `Cargo.toml`
4. Update this README with the new version number
5. Run tests to verify: `cargo test && cargo build --target wasm32-unknown-unknown --release`

### Upstream Issue

This patch may be unnecessary if upstream chrono adds a feature to control wasm-bindgen inclusion, or if datalogic-rs allows configuring chrono features. Consider checking:
- https://github.com/chronotope/chrono/issues
- https://github.com/GoPlasmatic/datalogic-rs/issues
