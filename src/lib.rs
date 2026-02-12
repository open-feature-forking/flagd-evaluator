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
use std::sync::Once;

static PANIC_HOOK_INIT: Once = Once::new();

// WASM is single-threaded, so we can use RefCell for better semantics.
// For native targets (testing, library usage), we use Mutex for thread safety.

#[cfg(target_family = "wasm")]
mod wasm_evaluator {
    use super::*;
    use std::cell::RefCell;

    /// A wrapper that makes RefCell Sync for WASM.
    ///
    /// # Safety
    /// This is safe because WASM is guaranteed to be single-threaded.
    /// The Sync impl allows using RefCell in static context.
    struct SyncRefCell<T>(RefCell<T>);

    // SAFETY: WASM is single-threaded, so this is safe.
    unsafe impl<T> Sync for SyncRefCell<T> {}

    static WASM_EVALUATOR: SyncRefCell<Option<evaluator::FlagEvaluator>> =
        SyncRefCell(RefCell::new(None));

    /// Get or initialize the global WASM evaluator instance.
    pub fn with_evaluator<F, R>(f: F) -> R
    where
        F: FnOnce(&mut evaluator::FlagEvaluator) -> R,
    {
        let mut borrow = WASM_EVALUATOR.0.borrow_mut();
        let evaluator =
            borrow.get_or_insert_with(|| evaluator::FlagEvaluator::new(ValidationMode::Strict));
        f(evaluator)
    }
}

#[cfg(not(target_family = "wasm"))]
mod wasm_evaluator {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static WASM_EVALUATOR: OnceLock<Mutex<evaluator::FlagEvaluator>> = OnceLock::new();

    /// Get or initialize the global evaluator instance (thread-safe for native).
    pub fn with_evaluator<F, R>(f: F) -> R
    where
        F: FnOnce(&mut evaluator::FlagEvaluator) -> R,
    {
        let mutex = WASM_EVALUATOR
            .get_or_init(|| Mutex::new(evaluator::FlagEvaluator::new(ValidationMode::Strict)));
        let mut guard = mutex.lock().unwrap();
        f(&mut guard)
    }
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
pub mod evaluator;
pub mod memory;
pub mod model;
pub mod operators;
pub mod types;
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
pub use evaluator::{FlagEvaluator, ValidationMode};
pub use memory::{
    bytes_to_memory, pack_ptr_len, string_from_memory, string_to_memory, unpack_ptr_len,
    wasm_alloc, wasm_dealloc,
};
pub use model::{FeatureFlag, ParsingResult, UpdateStateResponse};
pub use operators::create_evaluator;
pub use types::{ErrorCode, EvaluationResult, ResolutionReason};
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
    wasm_evaluator::with_evaluator(|eval| {
        eval.set_validation_mode(validation_mode);
    });

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
    wasm_evaluator::with_evaluator(|eval| {
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
    })
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
/// - The input buffers (flag_key and context) are freed by this function - caller should NOT dealloc them
/// - For empty context, pass context_ptr=0 and context_len=0 to skip allocation entirely
#[no_mangle]
pub extern "C" fn evaluate(
    flag_key_ptr: *mut u8,
    flag_key_len: u32,
    context_ptr: *mut u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_internal(
        flag_key_ptr as *const u8,
        flag_key_len,
        context_ptr as *const u8,
        context_len,
    );

    // Free input buffers - caller no longer needs to call dealloc for these
    // This saves 2 WASM calls per evaluation
    wasm_dealloc(flag_key_ptr, flag_key_len);
    // Only dealloc context if it was actually allocated (non-null pointer with length > 0)
    if !context_ptr.is_null() && context_len > 0 {
        wasm_dealloc(context_ptr, context_len);
    }

    string_to_memory(&result.to_json_string())
}

/// Evaluates a feature flag using pre-allocated buffers (no input deallocation).
///
/// This is a high-performance variant designed for use with pre-allocated buffers.
/// Unlike `evaluate` and `evaluate_binary`, this function does NOT deallocate the
/// input buffers, allowing them to be reused across multiple evaluations.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the JSON-encoded EvaluationResult string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller manages the input buffer lifecycle (they are NOT freed by this function)
/// - The caller will free the returned result memory using `dealloc`
/// - For empty context, pass context_ptr=0 and context_len=0
#[no_mangle]
pub extern "C" fn evaluate_reusable(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_internal(flag_key_ptr, flag_key_len, context_ptr, context_len);

    // NOTE: We do NOT deallocate input buffers here - caller manages them
    // This allows buffer reuse across multiple evaluations

    // Return JSON string (simple and sufficient for small result payloads)
    string_to_memory(&result.to_json_string())
}

/// Evaluates a feature flag by numeric index with pre-enriched context.
///
/// This is a high-performance variant that:
/// - Uses O(1) Vec index lookup instead of string-based HashMap lookup
/// - Expects the context to be pre-enriched with `$flagd.*` and `targetingKey` by the host
/// - Does NOT deallocate input buffers (caller manages them)
///
/// # Arguments
/// * `flag_index` - Numeric index from the `flagIndices` map returned by `update_state`
/// * `context_ptr` - Pointer to the pre-enriched evaluation context JSON string
/// * `context_len` - Length of the context string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the JSON-encoded EvaluationResult string.
///
/// # Safety
/// The caller must ensure:
/// - `context_ptr` points to valid memory (or is null with context_len=0)
/// - The memory region is valid UTF-8
/// - The caller manages the input buffer lifecycle (NOT freed by this function)
/// - The caller will free the returned result memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_by_index(
    flag_index: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_by_index_internal(flag_index, context_ptr, context_len);
    string_to_memory(&result.to_json_string())
}

/// Internal implementation of evaluate_by_index.
fn evaluate_by_index_internal(
    flag_index: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> EvaluationResult {
    init_panic_hook();

    let result = std::panic::catch_unwind(|| {
        wasm_evaluator::with_evaluator(|eval| {
            if eval.get_state().is_none() {
                return EvaluationResult::error(
                    ErrorCode::FlagNotFound,
                    "Flag state not initialized. Call update_state first.",
                );
            }

            // Parse context (pre-enriched by host)
            let context: Value = if context_ptr.is_null() || context_len == 0 {
                Value::Null
            } else {
                // SAFETY: The caller guarantees valid memory regions
                let context_str = match unsafe { string_from_memory(context_ptr, context_len) } {
                    Ok(s) => s,
                    Err(e) => {
                        return EvaluationResult::error(
                            ErrorCode::ParseError,
                            format!("Failed to read context: {}", e),
                        )
                    }
                };

                match serde_json::from_str(&context_str) {
                    Ok(v) => v,
                    Err(e) => {
                        return EvaluationResult::error(
                            ErrorCode::ParseError,
                            format!("Failed to parse context JSON: {}", e),
                        )
                    }
                }
            };

            eval.evaluate_flag_by_index(flag_index, &context)
        })
    });

    result.unwrap_or_else(|panic_err| {
        let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
            format!("Evaluation panic: {}", s)
        } else if let Some(s) = panic_err.downcast_ref::<String>() {
            format!("Evaluation panic: {}", s)
        } else {
            "Evaluation panic: unknown error".to_string()
        };
        EvaluationResult::error(ErrorCode::General, msg)
    })
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
        wasm_evaluator::with_evaluator(|eval| {
            // Check if state is initialized
            if eval.get_state().is_none() {
                return EvaluationResult::error(
                    ErrorCode::FlagNotFound,
                    "Flag state not initialized. Call update_state first.",
                );
            }

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

            let flag = eval.get_state().unwrap().flags.get(&flag_key);

            // Parse protobuf context
            let context: Value = if context_ptr.is_null()
                || context_len == 0
                || flag.is_some_and(|f| f.targeting.is_none())
            {
                Value::Null
            } else {
                let context_str = match unsafe { string_from_memory(context_ptr, context_len) } {
                    Ok(s) => s,
                    Err(e) => {
                        return EvaluationResult::error(
                            ErrorCode::ParseError,
                            format!("Failed to read context: {}", e),
                        )
                    }
                };

                match serde_json::from_str(&context_str) {
                    Ok(v) => v,
                    Err(e) => {
                        return EvaluationResult::error(
                            ErrorCode::ParseError,
                            format!("Failed to parse context JSON: {}", e),
                        )
                    }
                }
            };

            // Evaluate using the evaluator instance
            eval.evaluate_flag(&flag_key, &context)
        })
    });

    result.unwrap_or_else(|panic_err| {
        let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
            format!("Evaluation panic: {}", s)
        } else if let Some(s) = panic_err.downcast_ref::<String>() {
            format!("Evaluation panic: {}", s)
        } else {
            "Evaluation panic: unknown error".to_string()
        };
        EvaluationResult::error(ErrorCode::General, msg)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ============================================================================
    // Library tests using FlagEvaluator directly
    // ============================================================================

    #[test]
    fn test_evaluator_update_state_and_evaluate() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

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

        let response = evaluator.update_state(config).unwrap();
        assert!(response.success);

        let result = evaluator.evaluate_bool("boolFlag", &json!({}));
        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::Static);
    }

    #[test]
    fn test_evaluator_int_flag() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

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

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_int("intFlag", &json!({}));
        assert_eq!(result.value, json!(10));
        assert_eq!(result.variant, Some("small".to_string()));
    }

    #[test]
    fn test_evaluator_float_flag() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "pi": 3.14,
                        "e": 2.71
                    },
                    "defaultVariant": "pi"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_float("floatFlag", &json!({}));
        assert_eq!(result.value, json!(3.14));
        assert_eq!(result.variant, Some("pi".to_string()));
    }

    #[test]
    fn test_evaluator_string_flag() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "hello": "Hello, World!",
                        "goodbye": "Goodbye!"
                    },
                    "defaultVariant": "hello"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_string("stringFlag", &json!({}));
        assert_eq!(result.value, json!("Hello, World!"));
        assert_eq!(result.variant, Some("hello".to_string()));
    }

    #[test]
    fn test_evaluator_object_flag() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "objectFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "config1": {"key": "value1"},
                        "config2": {"key": "value2"}
                    },
                    "defaultVariant": "config1"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_flag("objectFlag", &json!({}));
        assert_eq!(result.value, json!({"key": "value1"}));
        assert_eq!(result.variant, Some("config1".to_string()));
    }

    #[test]
    fn test_evaluator_with_targeting() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "targetedFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "admin": "admin-value",
                        "user": "user-value"
                    },
                    "defaultVariant": "user",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "role"}, "admin"]},
                            "admin",
                            "user"
                        ]
                    }
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        // Test admin role
        let result_admin = evaluator.evaluate_flag("targetedFlag", &json!({"role": "admin"}));
        assert_eq!(result_admin.value, json!("admin-value"));
        assert_eq!(result_admin.variant, Some("admin".to_string()));
        assert_eq!(result_admin.reason, ResolutionReason::TargetingMatch);

        // Test user role
        let result_user = evaluator.evaluate_flag("targetedFlag", &json!({"role": "user"}));
        assert_eq!(result_user.value, json!("user-value"));
        assert_eq!(result_user.variant, Some("user".to_string()));
        assert_eq!(result_user.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_evaluator_disabled_flag() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "disabledFlag": {
                    "state": "DISABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_bool("disabledFlag", &json!({}));
        assert_eq!(result.value, Value::Null);
        assert_eq!(result.reason, ResolutionReason::Disabled);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
    }

    #[test]
    fn test_evaluator_flag_not_found() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "existingFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_flag("nonexistentFlag", &json!({}));
        assert_eq!(result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
    }

    #[test]
    fn test_evaluator_invalid_json_config() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let invalid_config = r#"{"flags": invalid}"#;
        let result = evaluator.update_state(invalid_config);
        assert!(result.is_ok()); // Returns Ok with error in response
        let response = result.unwrap();
        assert!(!response.success);
        assert!(response.error.is_some());
    }

    #[test]
    fn test_evaluator_fractional_targeting() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

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
                            ["control", 50],
                            ["treatment", 50]
                        ]
                    }
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_flag("abTestFlag", &json!({"targetingKey": "user-123"}));
        assert!(
            result.value == json!("control-experience")
                || result.value == json!("treatment-experience")
        );
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_evaluator_missing_targeting_key() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

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
                            ["a", 50],
                            ["b", 50]
                        ]
                    }
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        // Missing targetingKey - should use empty string
        let result = evaluator.evaluate_flag("testFlag", &json!({}));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
        assert!(result.value == json!("variant-a") || result.value == json!("variant-b"));
    }

    #[test]
    fn test_evaluator_unknown_variant_from_targeting() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "brokenFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "valid": "valid-value"
                    },
                    "defaultVariant": "valid",
                    "targeting": {
                        "if": [
                            true,
                            "unknown-variant",
                            "valid"
                        ]
                    }
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_flag("brokenFlag", &json!({}));
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::General));
    }

    #[test]
    fn test_evaluator_complex_targeting() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

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

        evaluator.update_state(config).unwrap();

        // Admin email
        let result_admin = evaluator.evaluate_flag(
            "complexFlag",
            &json!({"email": "admin@example.com", "appVersion": "1.0.0"}),
        );
        assert_eq!(result_admin.value, json!("premium-tier"));

        // Non-admin with new version
        let result_standard = evaluator.evaluate_flag(
            "complexFlag",
            &json!({"email": "user@example.com", "appVersion": "2.1.0"}),
        );
        assert_eq!(result_standard.value, json!("standard-tier"));

        // Non-admin with old version
        let result_basic = evaluator.evaluate_flag(
            "complexFlag",
            &json!({"email": "user@example.com", "appVersion": "1.5.0"}),
        );
        assert_eq!(result_basic.value, json!("basic-tier"));
    }

    #[test]
    fn test_evaluator_flagd_properties() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "enrichedFlag": {
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
                                    {"==": [{"var": "$flagd.flagKey"}, "enrichedFlag"]},
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

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_flag("enrichedFlag", &json!({}));
        assert_eq!(result.value, json!("properties-present"));
        assert_eq!(result.variant, Some("verified".to_string()));
    }

    #[test]
    fn test_evaluator_type_checking_bool() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {"val": "string-value", "alt": "alternative-value"},
                    "defaultVariant": "val"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_bool("stringFlag", &json!({}));
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
    }

    #[test]
    fn test_evaluator_type_checking_string() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {"val": true, "alt": false},
                    "defaultVariant": "val"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_string("stringFlag", &json!({}));
        assert_eq!(result.reason, ResolutionReason::FlagNotFound);
    }

    #[test]
    fn test_evaluator_type_checking_int() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {"val": 3.14, "alt": 2.71},
                    "defaultVariant": "val"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        // Integer evaluation should accept floats via coercion
        let result = evaluator.evaluate_int("floatFlag", &json!({}));
        assert_eq!(result.value, json!(3));
    }

    #[test]
    fn test_evaluator_type_checking_float() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {"val": 42, "alt": 100},
                    "defaultVariant": "val"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        // Float evaluation should accept integers via coercion
        let result = evaluator.evaluate_float("intFlag", &json!({}));
        assert_eq!(result.value, json!(42.0));
    }

    #[test]
    fn test_evaluator_type_checking_object() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {"val": "string-value", "alt": "alternative-value"},
                    "defaultVariant": "val"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let result = evaluator.evaluate_object("stringFlag", &json!({}));
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
    }

    #[test]
    fn test_evaluator_all_types_flag_not_found() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "existingFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let bool_result = evaluator.evaluate_bool("missingFlag", &json!({}));
        assert_eq!(bool_result.reason, ResolutionReason::FlagNotFound);

        let string_result = evaluator.evaluate_string("missingFlag", &json!({}));
        assert_eq!(string_result.reason, ResolutionReason::FlagNotFound);

        let int_result = evaluator.evaluate_int("missingFlag", &json!({}));
        assert_eq!(int_result.reason, ResolutionReason::FlagNotFound);

        let float_result = evaluator.evaluate_float("missingFlag", &json!({}));
        assert_eq!(float_result.reason, ResolutionReason::FlagNotFound);

        let object_result = evaluator.evaluate_object("missingFlag", &json!({}));
        assert_eq!(object_result.reason, ResolutionReason::FlagNotFound);
    }

    #[test]
    fn test_evaluator_disabled_flags_all_types() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "disabledBool": {
                    "state": "DISABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                },
                "disabledString": {
                    "state": "DISABLED",
                    "variants": {"val": "text", "alt": "alternative"},
                    "defaultVariant": "val"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        let bool_result = evaluator.evaluate_bool("disabledBool", &json!({}));
        assert_eq!(bool_result.value, Value::Null);
        assert_eq!(bool_result.reason, ResolutionReason::Disabled);

        let string_result = evaluator.evaluate_string("disabledString", &json!({}));
        assert_eq!(string_result.value, Value::Null);
        assert_eq!(string_result.reason, ResolutionReason::Disabled);
    }

    #[test]
    fn test_evaluator_validation_modes() {
        // Strict mode
        let mut strict_eval = FlagEvaluator::new(ValidationMode::Strict);
        assert_eq!(strict_eval.validation_mode(), ValidationMode::Strict);

        // Permissive mode
        let permissive_eval = FlagEvaluator::new(ValidationMode::Permissive);
        assert_eq!(
            permissive_eval.validation_mode(),
            ValidationMode::Permissive
        );

        // Change mode
        strict_eval.set_validation_mode(ValidationMode::Permissive);
        assert_eq!(strict_eval.validation_mode(), ValidationMode::Permissive);
    }

    #[test]
    fn test_evaluator_clear_state() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();
        assert!(evaluator.get_state().is_some());

        evaluator.clear_state();
        assert!(evaluator.get_state().is_none());
    }

    #[test]
    fn test_evaluator_changed_flags_detection() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Strict);

        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let response1 = evaluator.update_state(config1).unwrap();
        assert!(response1.changed_flags.is_some());
        assert_eq!(response1.changed_flags.unwrap(), vec!["flag1"]);

        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                },
                "flag2": {
                    "state": "ENABLED",
                    "variants": {"off": false, "on": true},
                    "defaultVariant": "off"
                }
            }
        }"#;

        let response2 = evaluator.update_state(config2).unwrap();
        assert!(response2.changed_flags.is_some());
        assert_eq!(response2.changed_flags.unwrap(), vec!["flag2"]);
    }
}

// ============================================================================
// WASM Integration Tests
// ============================================================================

#[cfg(test)]
mod wasm_tests {
    use super::*;
    use serde_json::json;

    /// Helper function to reset the WASM singleton evaluator between tests
    fn reset_wasm_evaluator() {
        wasm_evaluator::with_evaluator(|eval| {
            eval.clear_state();
            eval.set_validation_mode(ValidationMode::Strict);
        });
    }

    /// Helper to call update_state WASM export
    fn update_state_wasm(config: &str) -> String {
        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32)
    }

    /// Helper to call evaluate WASM export
    fn evaluate_wasm(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();

        let packed = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // evaluate_internal already returns EvaluationResult directly
        packed
    }

    #[test]
    fn test_wasm_memory_allocation() {
        // Test alloc and dealloc
        let size = 100;
        let ptr = wasm_alloc(size);
        assert!(!ptr.is_null());

        // Write some data
        unsafe {
            std::ptr::write_bytes(ptr, 0x42, size as usize);
        }

        // Free the memory
        wasm_dealloc(ptr, size);
    }

    #[test]
    fn test_wasm_update_state_export() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let response_json = update_state_wasm(config);
        let response: Value = serde_json::from_str(&response_json).unwrap();
        assert_eq!(response["success"], true);
    }

    #[test]
    fn test_wasm_evaluate_export() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "on"
                }
            }
        }"#;

        update_state_wasm(config);

        let result = evaluate_wasm("boolFlag", "{}");
        assert_eq!(result.value, json!(true));
        assert_eq!(result.reason, ResolutionReason::Static);
    }

    #[test]
    fn test_wasm_packed_pointer_format() {
        // Test pack and unpack utilities
        let ptr = 0x12345678 as *mut u8;
        let len = 42;

        let packed = pack_ptr_len(ptr, len);
        let (unpacked_ptr, unpacked_len) = unpack_ptr_len(packed);

        assert_eq!(unpacked_ptr, ptr);
        assert_eq!(unpacked_len, len);
    }

    #[test]
    fn test_wasm_utf8_handling() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "unicodeFlag": {
                    "state": "ENABLED",
                    "variants": {"emoji": "Hello 👋 World 🌍"},
                    "defaultVariant": "emoji"
                }
            }
        }"#;

        update_state_wasm(config);

        let result = evaluate_wasm("unicodeFlag", "{}");
        assert_eq!(result.value, json!("Hello 👋 World 🌍"));
    }

    #[test]
    fn test_wasm_evaluate_by_index() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "role"}, "admin"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let response_json = update_state_wasm(config);
        let response: Value = serde_json::from_str(&response_json).unwrap();
        assert!(response["success"].as_bool().unwrap());

        // Get the flag index from the response
        let flag_indices = response["flagIndices"].as_object().unwrap();
        let bool_flag_index = flag_indices["boolFlag"].as_u64().unwrap() as u32;

        // Test with pre-enriched context (matching)
        let context = json!({
            "role": "admin",
            "targetingKey": "user-1",
            "$flagd": {
                "flagKey": "boolFlag",
                "timestamp": 1234567890
            }
        });
        let context_str = context.to_string();
        let context_bytes = context_str.as_bytes();

        let result = evaluate_by_index_internal(
            bool_flag_index,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );
        assert_eq!(result.value, json!(true));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);

        // Test with non-matching context
        let context2 = json!({
            "role": "user",
            "targetingKey": "user-2",
            "$flagd": {
                "flagKey": "boolFlag",
                "timestamp": 1234567890
            }
        });
        let context2_str = context2.to_string();
        let context2_bytes = context2_str.as_bytes();

        let result2 = evaluate_by_index_internal(
            bool_flag_index,
            context2_bytes.as_ptr(),
            context2_bytes.len() as u32,
        );
        assert_eq!(result2.value, json!(false));
    }

    #[test]
    fn test_wasm_evaluate_by_index_invalid_index() {
        reset_wasm_evaluator();

        let config = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            }
        }"#;

        update_state_wasm(config);

        let result = evaluate_by_index_internal(999, std::ptr::null(), 0);
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
    }
}

// ============================================================================
// Context Key Extraction and Index Mapping Tests
// ============================================================================

#[cfg(test)]
mod optimization_tests {
    use super::*;
    use crate::evaluator::extract_required_context_keys;
    use serde_json::json;

    #[test]
    fn test_extract_keys_simple_var() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true, "off": false},
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

        let response = evaluator.update_state(config).unwrap();
        let keys = response.required_context_keys.unwrap();
        let flag_keys = keys.get("testFlag").unwrap();
        assert!(flag_keys.contains(&"email".to_string()));
        assert!(flag_keys.contains(&"targetingKey".to_string()));
    }

    #[test]
    fn test_extract_keys_multiple_vars() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "complexFlag": {
                    "state": "ENABLED",
                    "variants": {"premium": true, "standard": false, "basic": false},
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

        let response = evaluator.update_state(config).unwrap();
        let keys = response.required_context_keys.unwrap();
        let flag_keys = keys.get("complexFlag").unwrap();
        assert!(flag_keys.contains(&"email".to_string()));
        assert!(flag_keys.contains(&"appVersion".to_string()));
        assert!(flag_keys.contains(&"targetingKey".to_string()));
    }

    #[test]
    fn test_extract_keys_ignores_flagd_paths() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "enrichedFlag": {
                    "state": "ENABLED",
                    "variants": {"yes": true, "no": false},
                    "defaultVariant": "no",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "$flagd.flagKey"}, "enrichedFlag"]},
                            "yes",
                            "no"
                        ]
                    }
                }
            }
        }"#;

        let response = evaluator.update_state(config).unwrap();
        let keys = response.required_context_keys.unwrap();
        let flag_keys = keys.get("enrichedFlag").unwrap();
        // Should only have targetingKey, not $flagd
        assert!(!flag_keys.contains(&"$flagd".to_string()));
        assert!(flag_keys.contains(&"targetingKey".to_string()));
    }

    #[test]
    fn test_extract_keys_empty_var_returns_none() {
        // When a rule uses {"var": ""}, it accesses the entire context
        // so we can't filter keys
        let engine = create_evaluator();
        let rule = json!({"var": ""});
        let compiled = engine.compile(&rule).unwrap();
        let result = extract_required_context_keys(&compiled);
        assert!(result.is_none());
    }

    #[test]
    fn test_flag_indices_assigned() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "flagB": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                },
                "flagA": {
                    "state": "ENABLED",
                    "variants": {"off": false},
                    "defaultVariant": "off"
                }
            }
        }"#;

        let response = evaluator.update_state(config).unwrap();
        let indices = response.flag_indices.unwrap();

        // Indices should be assigned in sorted order
        assert_eq!(*indices.get("flagA").unwrap(), 0);
        assert_eq!(*indices.get("flagB").unwrap(), 1);
    }

    #[test]
    fn test_evaluate_by_index_matches_evaluate_flag() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "targetedFlag": {
                    "state": "ENABLED",
                    "variants": {"admin": "admin-value", "user": "user-value"},
                    "defaultVariant": "user",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "role"}, "admin"]},
                            "admin",
                            "user"
                        ]
                    }
                }
            }
        }"#;

        let response = evaluator.update_state(config).unwrap();
        let indices = response.flag_indices.unwrap();
        let index = *indices.get("targetedFlag").unwrap();

        // Create pre-enriched context
        let context = json!({
            "role": "admin",
            "targetingKey": "user-1",
            "$flagd": {
                "flagKey": "targetedFlag",
                "timestamp": 1234567890
            }
        });

        let result_by_index = evaluator.evaluate_flag_by_index(index, &context);
        let result_by_key = evaluator.evaluate_flag("targetedFlag", &json!({"role": "admin"}));

        assert_eq!(result_by_index.value, result_by_key.value);
        assert_eq!(result_by_index.variant, result_by_key.variant);
        assert_eq!(result_by_index.reason, result_by_key.reason);
    }

    #[test]
    fn test_evaluate_flag_pre_enriched() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "variants": {"yes": "found-key", "no": "no-key"},
                    "defaultVariant": "no",
                    "targeting": {
                        "if": [
                            {"!=": [{"var": "targetingKey"}, ""]},
                            "yes",
                            "no"
                        ]
                    }
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        // Pre-enriched context (has $flagd)
        let context = json!({
            "targetingKey": "user-123",
            "$flagd": {
                "flagKey": "myFlag",
                "timestamp": 1234567890
            }
        });

        let result = evaluator.evaluate_flag_pre_enriched("myFlag", &context);
        assert_eq!(result.value, json!("found-key"));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_evaluate_flag_pre_enriched_falls_back_to_normal() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "variants": {"yes": "found-key", "no": "no-key"},
                    "defaultVariant": "no",
                    "targeting": {
                        "if": [
                            {"!=": [{"var": "targetingKey"}, ""]},
                            "yes",
                            "no"
                        ]
                    }
                }
            }
        }"#;

        evaluator.update_state(config).unwrap();

        // Context without $flagd — should fall back to normal enrichment
        let context = json!({"targetingKey": "user-456"});
        let result = evaluator.evaluate_flag_pre_enriched("myFlag", &context);
        assert_eq!(result.value, json!("found-key"));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_static_flags_not_in_required_context_keys() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "staticFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                },
                "targetedFlag": {
                    "state": "ENABLED",
                    "variants": {"a": "val-a", "b": "val-b"},
                    "defaultVariant": "a",
                    "targeting": {
                        "if": [{"==": [{"var": "tier"}, "premium"]}, "b", "a"]
                    }
                }
            }
        }"#;

        let response = evaluator.update_state(config).unwrap();
        let keys = response.required_context_keys.unwrap();

        // Static flags should not appear in required_context_keys
        assert!(!keys.contains_key("staticFlag"));
        // Targeted flags should
        assert!(keys.contains_key("targetedFlag"));
        let targeted_keys = keys.get("targetedFlag").unwrap();
        assert!(targeted_keys.contains(&"tier".to_string()));
    }

    #[test]
    fn test_fractional_operator_includes_targeting_key() {
        let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);

        let config = r#"{
            "flags": {
                "abTestFlag": {
                    "state": "ENABLED",
                    "variants": {"control": "control", "treatment": "treatment"},
                    "defaultVariant": "control",
                    "targeting": {
                        "fractional": [
                            ["control", 50],
                            ["treatment", 50]
                        ]
                    }
                }
            }
        }"#;

        let response = evaluator.update_state(config).unwrap();
        let keys = response.required_context_keys.unwrap();
        let flag_keys = keys.get("abTestFlag").unwrap();
        // targetingKey is always included
        assert!(flag_keys.contains(&"targetingKey".to_string()));
    }
}
