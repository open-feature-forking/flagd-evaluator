//! # flagd-evaluator
//!
//! A WebAssembly-based JSON Logic evaluator with custom operators for feature flag evaluation.
//!
//! This library is designed to work with Chicory (pure Java WebAssembly runtime) and other
//! WASM runtimes. It provides a minimal API for evaluating JSON Logic rules with support for
//! custom operators like `fractional` for A/B testing.
//!
//! ## Features
//!
//! - **JSON Logic Evaluation**: Full support for standard JSON Logic operations via `datalogic-rs`
//! - **Custom Operators**: Support for feature-flag specific operators like `fractional` and
//!   `sem_ver` - all registered via the `datalogic_rs::Operator` trait. Additional operators
//!   like `starts_with` and `ends_with` are provided by datalogic-rs.
//! - **Feature Flag Evaluation**: State-based flag evaluation following the flagd provider specification
//! - **Memory Safe**: Clean memory management with explicit alloc/dealloc functions
//! - **Zero JNI**: Works with pure Java WASM runtimes like Chicory
//!
//! ## Exported Functions
//!
//! - `evaluate_logic`: Evaluates JSON Logic rules directly
//! - `update_state`: Updates the feature flag configuration state
//! - `evaluate`: Evaluates a feature flag against context (requires prior `update_state` call)
//! - `wasm_alloc`: Allocate memory from WASM linear memory
//! - `wasm_dealloc`: Free allocated memory
//!
//! ## Example
//!
//! ```ignore
//! // From Java via Chicory:
//! // 1. Update state with flag configuration
//! // 2. Allocate memory for flag key and context strings
//! // 3. Copy strings to WASM memory
//! // 4. Call evaluate with pointers
//! // 5. Parse the returned JSON result
//! // 6. Free allocated memory
//! ```

use std::panic;
use std::sync::{Mutex, Once, OnceLock};

static PANIC_HOOK_INIT: Once = Once::new();

/// Global singleton FlagEvaluator instance for WASM.
///
/// WASM is single-threaded, so we use a simple Mutex for interior mutability.
/// This provides a single global evaluator instance that all WASM exports use.
static WASM_EVALUATOR: OnceLock<Mutex<evaluator::FlagEvaluator>> = OnceLock::new();

/// Get or initialize the global WASM evaluator instance.
fn get_wasm_evaluator() -> &'static Mutex<evaluator::FlagEvaluator> {
    WASM_EVALUATOR.get_or_init(|| Mutex::new(evaluator::FlagEvaluator::new(ValidationMode::Strict)))
}

// Import optional host function for getting current time
// If the host doesn't provide this, we'll fall back to a default value
#[cfg(target_family = "wasm")]
#[link(wasm_import_module = "host")]
extern "C" {
    /// Gets the current Unix timestamp in seconds from the host environment.
    ///
    /// This function should be provided by the host (e.g., Java/Chicory) to supply
    /// the current time for $flagd.timestamp context enrichment.
    ///
    /// # Returns
    /// Unix timestamp in seconds since epoch (1970-01-01 00:00:00 UTC)
    #[link_name = "get_current_time_unix_seconds"]
    fn host_get_current_time() -> u64;
}

/// Initialize panic hook to prevent unreachable instructions in WASM
fn init_panic_hook() {
    PANIC_HOOK_INIT.call_once(|| {
        panic::set_hook(Box::new(|panic_info| {
            let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                *s
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic"
            };

            let location = if let Some(location) = panic_info.location() {
                format!(
                    " at {}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            } else {
                String::new()
            };

            // This will be visible in Chicory's error output
            eprintln!("PANIC in WASM module: {}{}", msg, location);
        }));
    });
}

pub mod error;
pub mod evaluation;
pub mod evaluator;
pub mod memory;
pub mod model;
pub mod operators;
pub mod validation;

/// Gets the current Unix timestamp in seconds.
///
/// This function attempts to call the host-provided `get_current_time_unix_seconds` function.
/// If the host doesn't provide this function (linking error), or if calling it fails,
/// it defaults to returning 0.
///
/// # Returns
/// Unix timestamp in seconds, or 0 if unavailable
pub fn get_current_time() -> u64 {
    #[cfg(target_family = "wasm")]
    {
        // In WASM, try to call the host function
        // If it's not provided, this will cause a link error that we catch
        std::panic::catch_unwind(|| unsafe { host_get_current_time() }).unwrap_or(0)
    }
    #[cfg(not(target_family = "wasm"))]
    {
        // In native code (tests, CLI), use SystemTime
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

use serde_json::Value;

pub use error::{ErrorType, EvaluatorError};
pub use evaluation::{
    evaluate_bool_flag, evaluate_flag, evaluate_float_flag, evaluate_int_flag,
    evaluate_object_flag, evaluate_string_flag, ErrorCode, EvaluationResult, ResolutionReason,
};
pub use evaluator::{FlagEvaluator, ValidationMode};
pub use memory::{
    pack_ptr_len, string_from_memory, string_to_memory, unpack_ptr_len, wasm_alloc, wasm_dealloc,
};
pub use model::{FeatureFlag, ParsingResult, UpdateStateResponse};
pub use operators::create_evaluator;
pub use validation::{validate_flags_config, ValidationError, ValidationResult};

/// Re-exports for external access to allocation functions.
///
/// These are the primary memory management functions that should be used
/// by the host runtime (e.g., Java via Chicory).
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    wasm_alloc(len)
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, len: u32) {
    wasm_dealloc(ptr, len)
}

/// Sets the validation mode for flag state updates (WASM export).
///
/// This function controls how validation errors are handled when updating flag state.
///
/// # Arguments
/// * `mode` - Validation mode: 0 = Strict (reject invalid configs), 1 = Permissive (accept with warnings)
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the response JSON string.
///
/// # Response Format
/// ```json
/// {
///   "success": true|false,
///   "error": null|"error message"
/// }
/// ```
///
/// # Example (from Java via Chicory)
/// ```java
/// // Set to permissive mode (1)
/// long result = instance.export("set_validation_mode").apply(1L)[0];
///
/// // Set to strict mode (0) - this is the default
/// long result = instance.export("set_validation_mode").apply(0L)[0];
/// ```
///
/// # Safety
/// The caller must ensure:
/// - The mode value is either 0 (Strict) or 1 (Permissive)
/// - The caller will free the returned memory using `dealloc`
#[export_name = "set_validation_mode"]
pub extern "C" fn set_validation_mode_wasm(mode: u32) -> u64 {
    let validation_mode = match mode {
        0 => ValidationMode::Strict,
        1 => ValidationMode::Permissive,
        _ => {
            let response = serde_json::json!({
                "success": false,
                "error": "Invalid validation mode. Use 0 for Strict or 1 for Permissive."
            })
            .to_string();
            return string_to_memory(&response);
        }
    };

    // Update validation mode on the singleton evaluator
    let evaluator = get_wasm_evaluator();
    let mut eval = evaluator.lock().unwrap();
    eval.set_validation_mode(validation_mode);

    let response = serde_json::json!({
        "success": true,
        "error": null
    })
    .to_string();

    string_to_memory(&response)
}

/// Updates the feature flag state with a new configuration.
///
/// This function parses the provided JSON configuration and stores it in
/// thread-local storage for later evaluation. It also detects which flags
/// have changed by comparing the new configuration with the previous state.
///
/// # Arguments
/// * `config_ptr` - Pointer to the JSON configuration string in WASM memory
/// * `config_len` - Length of the JSON configuration string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the response JSON string. The response includes a list of changed flag keys.
///
/// # Response Format
/// ```json
/// {
///   "success": true|false,
///   "error": null|"error message",
///   "changedFlags": ["flag1", "flag2", ...]
/// }
/// ```
///
/// The `changedFlags` array contains the keys of all flags that were:
/// - Added (present in new config but not in old)
/// - Removed (present in old config but not in new)
/// - Mutated (default variant, targeting rules, or metadata changed)
///
/// # Safety
/// The caller must ensure:
/// - `config_ptr` points to valid memory
/// - The memory region is valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn update_state(config_ptr: *const u8, config_len: u32) -> u64 {
    let response = update_state_internal(config_ptr, config_len);
    string_to_memory(&response)
}

/// Internal implementation of update_state.
fn update_state_internal(config_ptr: *const u8, config_len: u32) -> String {
    // Initialize panic hook for better error messages
    init_panic_hook();

    // SAFETY: The caller guarantees valid memory regions
    let config_str = match unsafe { string_from_memory(config_ptr, config_len) } {
        Ok(s) => s,
        Err(e) => {
            return serde_json::json!({
                "success": false,
                "error": format!("Failed to read configuration: {}", e),
                "changedFlags": null
            })
            .to_string()
        }
    };

    // Parse and store the configuration using the singleton evaluator
    let evaluator = get_wasm_evaluator();
    let mut eval = evaluator.lock().unwrap();
    match eval.update_state(&config_str) {
        Ok(response) => {
            // Convert UpdateStateResponse to JSON
            serde_json::to_string(&response).unwrap_or_else(|e| {
                serde_json::json!({
                    "success": false,
                    "error": format!("Failed to serialize response: {}", e),
                    "changedFlags": null
                })
                .to_string()
            })
        }
        Err(e) => serde_json::json!({
            "success": false,
            "error": e,
            "changedFlags": null
        })
        .to_string(),
    }
}

/// Evaluates a feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it against the provided context.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Response Format
/// The response matches the flagd provider specification:
/// ```json
/// {
///   "value": <resolved_value>,
///   "variant": "variant_name",
///   "reason": "STATIC"|"DEFAULT"|"TARGETING_MATCH"|"DISABLED"|"ERROR"|"FLAG_NOT_FOUND",
///   "errorCode": "FLAG_NOT_FOUND"|"PARSE_ERROR"|"TYPE_MISMATCH"|"GENERAL",
///   "errorMessage": "error description"
/// }
/// ```
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_internal(flag_key_ptr, flag_key_len, context_ptr, context_len);
    string_to_memory(&result.to_json_string())
}

/// Internal implementation of evaluate.
fn evaluate_internal(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> EvaluationResult {
    // Initialize panic hook for better error messages
    init_panic_hook();

    // Catch any panics and convert them to error responses
    let result = std::panic::catch_unwind(|| {
        // SAFETY: The caller guarantees valid memory regions
        let flag_key = match unsafe { string_from_memory(flag_key_ptr, flag_key_len) } {
            Ok(s) => s,
            Err(e) => {
                return EvaluationResult::error(
                    ErrorCode::ParseError,
                    format!("Failed to read flag key: {}", e),
                )
            }
        };

        let context_str = match unsafe { string_from_memory(context_ptr, context_len) } {
            Ok(s) => s,
            Err(e) => {
                return EvaluationResult::error(
                    ErrorCode::ParseError,
                    format!("Failed to read context: {}", e),
                )
            }
        };

        // Parse the context JSON
        let context: Value = match serde_json::from_str(&context_str) {
            Ok(v) => v,
            Err(e) => {
                return EvaluationResult::error(
                    ErrorCode::ParseError,
                    format!("Failed to parse context JSON: {}", e),
                )
            }
        };

        // Get the singleton evaluator and evaluate the flag
        let evaluator = get_wasm_evaluator();
        let eval = evaluator.lock().unwrap();

        // Check if state is initialized
        if eval.get_state().is_none() {
            return EvaluationResult::error(
                ErrorCode::FlagNotFound,
                "Flag state not initialized. Call update_state first.",
            );
        }

        // Evaluate using the evaluator instance
        eval.evaluate_flag(&flag_key, &context)
    });

    match result {
        Ok(eval_result) => eval_result,
        Err(panic_err) => {
            let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                format!("Evaluation panic: {}", s)
            } else if let Some(s) = panic_err.downcast_ref::<String>() {
                format!("Evaluation panic: {}", s)
            } else {
                "Evaluation panic: unknown error".to_string()
            };
            EvaluationResult::error(ErrorCode::General, msg)
        }
    }
}

/// Evaluates a boolean feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as a boolean flag. If the resolved value is not a boolean, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_boolean(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        |eval, key, ctx| eval.evaluate_bool(key, ctx),
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates a string feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as a string flag. If the resolved value is not a string, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_string(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        |eval, key, ctx| eval.evaluate_string(key, ctx),
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates an integer feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as an integer flag. If the resolved value is not an integer, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_integer(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        |eval, key, ctx| eval.evaluate_int(key, ctx),
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates a float feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as a float flag. If the resolved value is not a number, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_float(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        |eval, key, ctx| eval.evaluate_float(key, ctx),
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates an object feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as an object flag. If the resolved value is not an object, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_object(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        |eval, key, ctx| eval.evaluate_flag(key, ctx),
    );
    string_to_memory(&result.to_json_string())
}

/// Internal helper function for type-specific evaluation.
///
/// This function handles the common logic for all typed evaluation functions:
/// parsing inputs and calling the type-specific evaluator method on the singleton.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the context JSON string
/// * `context_len` - Length of the context JSON string
/// * `evaluator_fn` - Function that calls the appropriate method on FlagEvaluator
fn evaluate_typed_internal<F>(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
    evaluator_fn: F,
) -> EvaluationResult
where
    F: Fn(&evaluator::FlagEvaluator, &str, &Value) -> EvaluationResult,
{
    // Initialize panic hook for better error messages
    init_panic_hook();

    // Catch any panics and convert them to error responses
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: The caller guarantees valid memory regions
        let flag_key = match unsafe { string_from_memory(flag_key_ptr, flag_key_len) } {
            Ok(s) => s,
            Err(e) => {
                return EvaluationResult::error(
                    ErrorCode::ParseError,
                    format!("Failed to read flag key: {}", e),
                )
            }
        };

        let context_str = match unsafe { string_from_memory(context_ptr, context_len) } {
            Ok(s) => s,
            Err(e) => {
                return EvaluationResult::error(
                    ErrorCode::ParseError,
                    format!("Failed to read context: {}", e),
                )
            }
        };

        // Parse the context JSON
        let context: Value = match serde_json::from_str(&context_str) {
            Ok(v) => v,
            Err(e) => {
                return EvaluationResult::error(
                    ErrorCode::ParseError,
                    format!("Failed to parse context JSON: {}", e),
                )
            }
        };

        // Get the singleton evaluator and evaluate using the type-specific method
        let evaluator = get_wasm_evaluator();
        let eval = evaluator.lock().unwrap();

        // Check if state is initialized
        if eval.get_state().is_none() {
            return EvaluationResult::error(
                ErrorCode::FlagNotFound,
                "Flag state not initialized. Call update_state first.",
            );
        }

        // Call the type-specific evaluator method
        evaluator_fn(&eval, &flag_key, &context)
    }));

    match result {
        Ok(eval_result) => eval_result,
        Err(panic_err) => {
            let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                format!("Evaluation panic: {}", s)
            } else if let Some(s) = panic_err.downcast_ref::<String>() {
                format!("Evaluation panic: {}", s)
            } else {
                "Evaluation panic: unknown error".to_string()
            };
            EvaluationResult::error(ErrorCode::General, msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper function to reset the WASM singleton evaluator between tests
    fn reset_wasm_evaluator() {
        let evaluator = get_wasm_evaluator();
        let mut eval = evaluator.lock().unwrap();
        eval.clear_state();
        eval.set_validation_mode(ValidationMode::Strict);
    }

    // ============================================================================
    // update_state and evaluate function tests
    // ============================================================================

    #[test]
    fn test_update_state_and_evaluate_bool() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        let update_response =
            update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);
        let update_json: Value = serde_json::from_str(&update_response).unwrap();
        assert_eq!(update_json["success"], true);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "boolFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::Static);
    }

    #[test]
    fn test_evaluate_int_flag() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "intFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(10));
        assert_eq!(result.variant, Some("small".to_string()));
    }

    #[test]
    fn test_evaluate_float_flag() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "low": 1.5,
                        "high": 9.99
                    },
                    "defaultVariant": "low"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "floatFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(1.5));
        assert_eq!(result.variant, Some("low".to_string()));
    }

    #[test]
    fn test_evaluate_string_flag() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson",
                        "blue": "azure"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "stringFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!("crimson"));
        assert_eq!(result.variant, Some("red".to_string()));
    }

    #[test]
    fn test_evaluate_object_flag() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "objectFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "config1": {"timeout": 30, "retries": 3},
                        "config2": {"timeout": 60, "retries": 5}
                    },
                    "defaultVariant": "config1"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "objectFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!({"timeout": 30, "retries": 3}));
        assert_eq!(result.variant, Some("config1".to_string()));
    }

    #[test]
    fn test_evaluate_with_targeting() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "targetedFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "email"}, "admin@example.com"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test matching context
        let context = r#"{"email": "admin@example.com"}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "targetedFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);

        // Test non-matching context
        let context = r#"{"email": "user@example.com"}"#;
        let context_bytes = context.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_evaluate_disabled_flag() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "disabledFlag": {
                    "state": "DISABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "disabledFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Disabled flags return null value with Disabled reason to signal "use code default"
        assert_eq!(result.value, Value::Null);
        assert_eq!(result.reason, ResolutionReason::Disabled);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
    }

    #[test]
    fn test_evaluate_flag_not_found() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "existingFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "nonexistentFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_update_state_invalid_json() {
        reset_wasm_evaluator();

        let config = "not valid json";
        let config_bytes = config.as_bytes();
        let response = update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);
        let json: Value = serde_json::from_str(&response).unwrap();

        assert_eq!(json["success"], false);
        assert!(json["error"].is_string());
    }

    #[test]
    fn test_evaluate_invalid_context_json() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "not valid json";
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::ParseError));
    }

    #[test]
    fn test_evaluate_with_fractional_targeting() {
        reset_wasm_evaluator();
        let evaluator = get_wasm_evaluator();
        evaluator
            .lock()
            .unwrap()
            .set_validation_mode(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "abTestFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "control": "control-experience",
                        "treatment": "treatment-experience"
                    },
                    "defaultVariant": "control",
                    "targeting": {
                        "fractional": [
                            {"var": "userId"},
                            ["control", 50, "treatment", 50]
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{"userId": "user-123"}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "abTestFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Result should be one of the variants
        assert!(
            result.value == json!("control-experience")
                || result.value == json!("treatment-experience")
        );
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_evaluation_result_serialization() {
        let result = EvaluationResult::static_result(json!(42), "variant1".to_string());
        let json_str = result.to_json_string();

        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["value"], 42);
        assert_eq!(parsed["variant"], "variant1");
        assert_eq!(parsed["reason"], "STATIC");
    }

    // ============================================================================
    // Edge case tests: missing targeting key, unknown variant, malformed expressions
    // ============================================================================

    #[test]
    fn test_edge_case_missing_targeting_key() {
        reset_wasm_evaluator();
        let evaluator = get_wasm_evaluator();
        evaluator
            .lock()
            .unwrap()
            .set_validation_mode(ValidationMode::Permissive);

        // Flag that uses targetingKey for fractional bucketing
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "a": "variant-a",
                        "b": "variant-b"
                    },
                    "defaultVariant": "a",
                    "targeting": {
                        "fractional": [
                            {"var": "targetingKey"},
                            ["a", 50, "b", 50]
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Context without targetingKey - should use empty string as default
        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Should succeed with one of the variants (using empty string as key)
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
        assert!(
            result.value == json!("variant-a") || result.value == json!("variant-b"),
            "Expected variant-a or variant-b, got: {:?}",
            result.value
        );
    }

    #[test]
    fn test_edge_case_unknown_variant_from_targeting() {
        reset_wasm_evaluator();

        // Targeting rule returns a variant name that doesn't exist
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "user"}, "admin"]},
                            "unknown_variant",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{"user": "admin"}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Unknown variant should return an error (Java-compatible behavior)
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::General));
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_edge_case_malformed_targeting_expression() {
        reset_wasm_evaluator();
        let evaluator = get_wasm_evaluator();
        evaluator
            .lock()
            .unwrap()
            .set_validation_mode(ValidationMode::Permissive);

        // Invalid JSON Logic expression (missing closing bracket)
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "invalid_operator": [1, 2, 3]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Should return error for malformed/unknown operator
        assert_eq!(result.reason, ResolutionReason::Error);
        // The error code might be General instead of ParseError due to unknown operator
        assert!(result.error_code.is_some());
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_edge_case_fractional_with_targetingkey_context() {
        reset_wasm_evaluator();
        let evaluator = get_wasm_evaluator();
        evaluator
            .lock()
            .unwrap()
            .set_validation_mode(ValidationMode::Permissive);

        // Use targetingKey for consistent bucketing
        let config = r#"{
            "flags": {
                "featureRollout": {
                    "state": "ENABLED",
                    "variants": {
                        "enabled": true,
                        "disabled": false
                    },
                    "defaultVariant": "disabled",
                    "targeting": {
                        "fractional": [
                            {"var": "targetingKey"},
                            ["enabled", 10, "disabled", 90]
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test with explicit targetingKey
        let context1 = r#"{"targetingKey": "user-001"}"#;
        let context_bytes1 = context1.as_bytes();
        let flag_key = "featureRollout";
        let flag_key_bytes = flag_key.as_bytes();

        let result1 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes1.as_ptr(),
            context_bytes1.len() as u32,
        );

        // Same targetingKey should give same result (consistency)
        let result2 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes1.as_ptr(),
            context_bytes1.len() as u32,
        );

        assert_eq!(result1.value, result2.value);
        assert_eq!(result1.variant, result2.variant);
        assert_eq!(result1.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_edge_case_targeting_with_flag_key_reference() {
        reset_wasm_evaluator();

        // Targeting rule that uses the $flagd.flagKey field
        let config = r#"{
            "flags": {
                "debugFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "$flagd.flagKey"}, "debugFlag"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "debugFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Should match because $flagd.flagKey is enriched in context
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_edge_case_complex_targeting_with_all_operators() {
        reset_wasm_evaluator();

        // Complex rule using multiple custom operators
        let config = r#"{
            "flags": {
                "complexFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "premium": "premium-tier",
                        "standard": "standard-tier",
                        "basic": "basic-tier"
                    },
                    "defaultVariant": "basic",
                    "targeting": {
                        "if": [
                            {"starts_with": [{"var": "email"}, "admin@"]},
                            "premium",
                            {
                                "if": [
                                    {"sem_ver": [{"var": "appVersion"}, ">=", "2.0.0"]},
                                    "standard",
                                    "basic"
                                ]
                            }
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test admin email - should get premium
        let context1 = r#"{"email": "admin@example.com", "appVersion": "1.0.0"}"#;
        let context_bytes1 = context1.as_bytes();
        let flag_key = "complexFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result1 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes1.as_ptr(),
            context_bytes1.len() as u32,
        );

        assert_eq!(result1.value, json!("premium-tier"));
        assert_eq!(result1.variant, Some("premium".to_string()));

        // Test non-admin with new version - should get standard
        let context2 = r#"{"email": "user@example.com", "appVersion": "2.1.0"}"#;
        let context_bytes2 = context2.as_bytes();

        let result2 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes2.as_ptr(),
            context_bytes2.len() as u32,
        );

        assert_eq!(result2.value, json!("standard-tier"));
        assert_eq!(result2.variant, Some("standard".to_string()));

        // Test non-admin with old version - should get basic
        let context3 = r#"{"email": "user@example.com", "appVersion": "1.5.0"}"#;
        let context_bytes3 = context3.as_bytes();

        let result3 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes3.as_ptr(),
            context_bytes3.len() as u32,
        );

        assert_eq!(result3.value, json!("basic-tier"));
        assert_eq!(result3.variant, Some("basic".to_string()));
    }

    #[test]
    fn test_flagd_timestamp_in_targeting() {
        reset_wasm_evaluator();

        // Flag that uses $flagd.timestamp for time-based targeting
        let config = r#"{
            "flags": {
                "timeBasedFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "current": true,
                        "expired": false
                    },
                    "defaultVariant": "expired",
                    "targeting": {
                        "if": [
                            {">": [{"var": "$flagd.timestamp"}, 1000000000]},
                            "current",
                            "expired"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "timeBasedFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Current timestamp should be > 1000000000 (Sep 2001), so should get "current"
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("current".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_flagd_properties_are_injected() {
        reset_wasm_evaluator();

        // Flag that verifies both $flagd properties exist
        let config = r#"{
            "flags": {
                "verifyFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "verified": "properties-present",
                        "failed": "properties-missing"
                    },
                    "defaultVariant": "failed",
                    "targeting": {
                        "if": [
                            {
                                "and": [
                                    {"==": [{"var": "$flagd.flagKey"}, "verifyFlag"]},
                                    {">": [{"var": "$flagd.timestamp"}, 0]}
                                ]
                            },
                            "verified",
                            "failed"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "verifyFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Both conditions should pass
        assert_eq!(result.value, json!("properties-present"));
        assert_eq!(result.variant, Some("verified".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    // ============================================================================
    // Type-specific WASM evaluation tests
    // ============================================================================

    fn evaluate_boolean_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            |eval, key, ctx| eval.evaluate_bool(key, ctx),
        )
    }

    fn evaluate_string_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            |eval, key, ctx| eval.evaluate_string(key, ctx),
        )
    }

    fn evaluate_integer_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            |eval, key, ctx| eval.evaluate_int(key, ctx),
        )
    }

    fn evaluate_float_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            |eval, key, ctx| eval.evaluate_float(key, ctx),
        )
    }

    fn evaluate_object_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            |eval, key, ctx| eval.evaluate_flag(key, ctx),
        )
    }

    #[test]
    fn test_evaluate_boolean_success() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_boolean_internal("boolFlag", "{}");
        assert_eq!(result.value, json!(true));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_boolean_type_mismatch() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": "yes",
                        "off": "no"
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_boolean_internal("stringFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected boolean"));
    }

    #[test]
    fn test_evaluate_string_success() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson",
                        "blue": "azure"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_string_internal("stringFlag", "{}");
        assert_eq!(result.value, json!("crimson"));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_string_type_mismatch() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_string_internal("intFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected string"));
    }

    #[test]
    fn test_evaluate_integer_success() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_integer_internal("intFlag", "{}");
        assert_eq!(result.value, json!(10));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_integer_type_mismatch_float() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "low": 1.5,
                        "high": 9.99
                    },
                    "defaultVariant": "low"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_integer_internal("floatFlag", "{}");
        // Float is coerced to integer (Java-compatible behavior)
        // 1.5 becomes 1
        assert_eq!(result.value, json!(1));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_float_success() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "low": 1.5,
                        "high": 9.99
                    },
                    "defaultVariant": "low"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_float_internal("floatFlag", "{}");
        assert_eq!(result.value, json!(1.5));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_float_accepts_integer() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_float_internal("intFlag", "{}");
        // Integer is coerced to float (Java-compatible behavior)
        assert_eq!(result.value, json!(10.0));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_float_type_mismatch() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_float_internal("stringFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected float"));
    }

    #[test]
    fn test_evaluate_object_success() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "objectFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "config1": {"timeout": 30, "retries": 3},
                        "config2": {"timeout": 60, "retries": 5}
                    },
                    "defaultVariant": "config1"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_object_internal("objectFlag", "{}");
        assert_eq!(result.value, json!({"timeout": 30, "retries": 3}));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_object_type_mismatch() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_object_internal("stringFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected object"));
    }

    #[test]
    fn test_all_type_evaluators_flag_not_found() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "existingFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // All type-specific evaluators should return FLAG_NOT_FOUND for missing flags
        let bool_result = evaluate_boolean_internal("missingFlag", "{}");
        assert_eq!(bool_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(bool_result.error_code, Some(ErrorCode::FlagNotFound));

        let string_result = evaluate_string_internal("missingFlag", "{}");
        assert_eq!(string_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(string_result.error_code, Some(ErrorCode::FlagNotFound));

        let int_result = evaluate_integer_internal("missingFlag", "{}");
        assert_eq!(int_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(int_result.error_code, Some(ErrorCode::FlagNotFound));

        let float_result = evaluate_float_internal("missingFlag", "{}");
        assert_eq!(float_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(float_result.error_code, Some(ErrorCode::FlagNotFound));

        let object_result = evaluate_object_internal("missingFlag", "{}");
        assert_eq!(object_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(object_result.error_code, Some(ErrorCode::FlagNotFound));
    }

    #[test]
    fn test_type_evaluators_with_targeting() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "email"}, "admin@example.com"]},
                            "on",
                            "off"
                        ]
                    }
                },
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small",
                    "targeting": {
                        "if": [
                            {">": [{"var": "age"}, 18]},
                            "large",
                            "small"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test boolean with targeting match
        let bool_result =
            evaluate_boolean_internal("boolFlag", r#"{"email": "admin@example.com"}"#);
        assert_eq!(bool_result.value, json!(true));
        assert_eq!(bool_result.reason, ResolutionReason::TargetingMatch);

        // Test boolean with targeting no match
        let bool_result = evaluate_boolean_internal("boolFlag", r#"{"email": "user@example.com"}"#);
        assert_eq!(bool_result.value, json!(false));
        assert_eq!(bool_result.reason, ResolutionReason::TargetingMatch);

        // Test integer with targeting
        let int_result = evaluate_integer_internal("intFlag", r#"{"age": 25}"#);
        assert_eq!(int_result.value, json!(100));
        assert_eq!(int_result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_type_evaluators_with_disabled_flags() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "disabledBool": {
                    "state": "DISABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off"
                },
                "disabledString": {
                    "state": "DISABLED",
                    "variants": {
                        "red": "crimson",
                        "blue": "azure"
                    },
                    "defaultVariant": "blue"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let bool_result = evaluate_boolean_internal("disabledBool", "{}");
        // Disabled flags return null value with Disabled reason to signal "use code default"
        assert_eq!(bool_result.value, Value::Null);
        assert_eq!(bool_result.reason, ResolutionReason::Disabled);
        assert_eq!(bool_result.error_code, Some(ErrorCode::FlagNotFound));

        let string_result = evaluate_string_internal("disabledString", "{}");
        // Disabled flags return null value with Disabled reason to signal "use code default"
        assert_eq!(string_result.value, Value::Null);
        assert_eq!(string_result.reason, ResolutionReason::Disabled);
        assert_eq!(string_result.error_code, Some(ErrorCode::FlagNotFound));
    }
}
