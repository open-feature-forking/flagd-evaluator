// pythonize::depythonize returns errors that auto-convert to PyErr via Into,
// which clippy flags as "useless conversion" when used with the ? operator.
#![allow(clippy::useless_conversion)]

use ::flagd_evaluator::{EvaluationResult, ValidationMode};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// FlagEvaluator - Stateful feature flag evaluator with host-side optimizations
///
/// This class maintains an internal state of feature flag configurations
/// and provides methods to evaluate flags against context data.
///
/// After `update_state()`, the evaluator caches:
/// - Pre-evaluated results for static/disabled flags (returned without calling Rust)
/// - Required context keys per flag (for filtered context serialization)
/// - Flag indices (for index-based evaluation that avoids flag key serialization)
///
/// Example:
///     >>> evaluator = FlagEvaluator()
///     >>> evaluator.update_state({
///     ...     "flags": {
///     ...         "myFlag": {
///     ...             "state": "ENABLED",
///     ...             "variants": {"on": True, "off": False},
///     ...             "defaultVariant": "on"
///     ...         }
///     ...     }
///     ... })
///     >>> result = evaluator.evaluate_bool("myFlag", {}, False)
///     >>> print(result)
///     True
#[pyclass]
struct FlagEvaluator {
    /// Wrap the Rust FlagEvaluator directly
    inner: ::flagd_evaluator::FlagEvaluator,

    /// Cache of pre-evaluated results for static/disabled flags.
    /// These flags always return the same result regardless of context,
    /// so we skip the Rust evaluation call entirely.
    pre_evaluated_cache: HashMap<String, EvaluationResult>,

    /// Per-flag required context keys for host-side filtering.
    /// When present, only serialize these keys (plus $flagd enrichment and targetingKey)
    /// instead of the full context dict.
    required_context_keys: HashMap<String, HashSet<String>>,

    /// Flag key to numeric index mapping for evaluate_by_index.
    /// Allows using O(1) Vec lookup on the Rust side instead of HashMap lookup.
    flag_indices: HashMap<String, u32>,
}

impl FlagEvaluator {
    /// Builds a filtered context Value containing only the required keys,
    /// plus $flagd enrichment and targetingKey.
    fn build_filtered_context(
        flag_key: &str,
        context: &Value,
        required_keys: &HashSet<String>,
    ) -> Value {
        let ctx_obj = context.as_object();
        let mut filtered = Map::new();

        // Copy only the required keys from the original context
        if let Some(obj) = ctx_obj {
            for key in required_keys {
                if key.starts_with("$flagd") {
                    continue;
                }
                if let Some(val) = obj.get(key) {
                    filtered.insert(key.clone(), val.clone());
                }
            }
        }

        // Ensure targetingKey is always present (default to empty string)
        if !filtered.contains_key("targetingKey") {
            let targeting_key = ctx_obj
                .and_then(|o| o.get("targetingKey"))
                .cloned()
                .unwrap_or(Value::String(String::new()));
            filtered.insert("targetingKey".to_string(), targeting_key);
        }

        // Add $flagd enrichment
        let timestamp = ::flagd_evaluator::get_current_time();
        let mut flagd_props = Map::new();
        flagd_props.insert("flagKey".to_string(), Value::String(flag_key.to_string()));
        flagd_props.insert("timestamp".to_string(), Value::Number(timestamp.into()));
        filtered.insert("$flagd".to_string(), Value::Object(flagd_props));

        Value::Object(filtered)
    }

    /// Evaluates a flag using the optimized path: pre-evaluated cache, filtered context,
    /// and index-based evaluation when possible. Falls back to full evaluation otherwise.
    fn evaluate_optimized(&self, flag_key: &str, context: &Value) -> EvaluationResult {
        // Fast path: return cached result for static/disabled flags
        if let Some(cached) = self.pre_evaluated_cache.get(flag_key) {
            return cached.clone();
        }

        // Check if we can use filtered context serialization
        if let Some(required_keys) = self.required_context_keys.get(flag_key) {
            let filtered_context = Self::build_filtered_context(flag_key, context, required_keys);

            // If we also have a flag index, use the index-based evaluation path
            if let Some(&index) = self.flag_indices.get(flag_key) {
                return self.inner.evaluate_flag_by_index(index, &filtered_context);
            }

            // Otherwise use pre-enriched evaluation (context already has $flagd)
            return self
                .inner
                .evaluate_flag_pre_enriched(flag_key, &filtered_context);
        }

        // Full evaluation path (no optimization data available for this flag)
        self.inner.evaluate_flag(flag_key, context)
    }
}

#[pymethods]
impl FlagEvaluator {
    /// Create a new FlagEvaluator instance
    ///
    /// Args:
    ///     permissive (bool, optional): If True, use permissive validation mode (accept invalid configs).
    ///                                   If False, use strict mode (reject invalid configs).
    ///                                   Defaults to False (strict mode).
    #[new]
    #[pyo3(signature = (permissive=false))]
    fn new(permissive: bool) -> Self {
        let mode = if permissive {
            ValidationMode::Permissive
        } else {
            ValidationMode::Strict
        };

        FlagEvaluator {
            inner: ::flagd_evaluator::FlagEvaluator::new(mode),
            pre_evaluated_cache: HashMap::new(),
            required_context_keys: HashMap::new(),
            flag_indices: HashMap::new(),
        }
    }

    /// Update the flag configuration state
    ///
    /// Parses the configuration, detects changed flags, and populates host-side
    /// optimization caches (pre-evaluated results, required context keys, flag indices).
    ///
    /// Args:
    ///     config (dict): Flag configuration in flagd format
    ///
    /// Returns:
    ///     dict: Update response with changed flag keys, pre-evaluated results,
    ///           required context keys, and flag indices
    fn update_state(&mut self, py: Python, config: &Bound<'_, PyDict>) -> PyResult<PyObject> {
        // Convert Python dict to JSON Value
        let config_value: Value = pythonize::depythonize(config.as_any())?;

        // Convert to JSON string for parsing
        let config_str = serde_json::to_string(&config_value).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to serialize config: {}",
                e
            ))
        })?;

        // Delegate to the Rust FlagEvaluator
        let response = self.inner.update_state(&config_str).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to update state: {}",
                e
            ))
        })?;

        // Update pre-evaluated cache
        self.pre_evaluated_cache = response.pre_evaluated.as_ref().cloned().unwrap_or_default();

        // Update required context keys cache
        self.required_context_keys = match &response.required_context_keys {
            Some(keys_map) => keys_map
                .iter()
                .map(|(k, v)| (k.clone(), v.iter().cloned().collect::<HashSet<String>>()))
                .collect(),
            None => HashMap::new(),
        };

        // Update flag indices cache
        self.flag_indices = response.flag_indices.as_ref().cloned().unwrap_or_default();

        // Convert response to Python dict
        pythonize::pythonize(py, &response)
            .map(|bound| bound.unbind())
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Failed to convert response: {}",
                    e
                ))
            })
    }

    /// Evaluate a feature flag
    ///
    /// Uses host-side optimizations when available:
    /// 1. Returns cached result for static/disabled flags (no Rust call)
    /// 2. Filters context to only required keys when possible
    /// 3. Uses index-based evaluation when both index and required keys are known
    ///
    /// Args:
    ///     flag_key (str): The flag key to evaluate
    ///     context (dict): Evaluation context
    ///
    /// Returns:
    ///     dict: Evaluation result with value, variant, reason, and metadata
    fn evaluate(
        &self,
        py: Python,
        flag_key: String,
        context: &Bound<'_, PyDict>,
    ) -> PyResult<PyObject> {
        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = self.evaluate_optimized(&flag_key, &context_value);

        // Convert result to Python dict
        pythonize::pythonize(py, &result)
            .map(|bound| bound.unbind())
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Failed to convert result: {}",
                    e
                ))
            })
    }

    /// Evaluate a boolean flag
    ///
    /// Args:
    ///     flag_key (str): The flag key to evaluate
    ///     context (dict): Evaluation context
    ///     default_value (bool): Default value if evaluation fails
    ///
    /// Returns:
    ///     bool: The evaluated boolean value
    fn evaluate_bool(
        &self,
        flag_key: String,
        context: &Bound<'_, PyDict>,
        default_value: bool,
    ) -> PyResult<bool> {
        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = self.evaluate_optimized(&flag_key, &context_value);

        if result.error_code.is_some() {
            return Ok(default_value);
        }

        match result.value {
            Value::Bool(b) => Ok(b),
            _ => Ok(default_value),
        }
    }

    /// Evaluate a string flag
    ///
    /// Args:
    ///     flag_key (str): The flag key to evaluate
    ///     context (dict): Evaluation context
    ///     default_value (str): Default value if evaluation fails
    ///
    /// Returns:
    ///     str: The evaluated string value
    fn evaluate_string(
        &self,
        flag_key: String,
        context: &Bound<'_, PyDict>,
        default_value: String,
    ) -> PyResult<String> {
        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = self.evaluate_optimized(&flag_key, &context_value);

        if result.error_code.is_some() {
            return Ok(default_value);
        }

        match result.value {
            Value::String(s) => Ok(s),
            _ => Ok(default_value),
        }
    }

    /// Evaluate an integer flag
    ///
    /// Args:
    ///     flag_key (str): The flag key to evaluate
    ///     context (dict): Evaluation context
    ///     default_value (int): Default value if evaluation fails
    ///
    /// Returns:
    ///     int: The evaluated integer value
    fn evaluate_int(
        &self,
        flag_key: String,
        context: &Bound<'_, PyDict>,
        default_value: i64,
    ) -> PyResult<i64> {
        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = self.evaluate_optimized(&flag_key, &context_value);

        if result.error_code.is_some() {
            return Ok(default_value);
        }

        match result.value {
            Value::Number(n) => Ok(n.as_i64().unwrap_or(default_value)),
            _ => Ok(default_value),
        }
    }

    /// Evaluate a float flag
    ///
    /// Args:
    ///     flag_key (str): The flag key to evaluate
    ///     context (dict): Evaluation context
    ///     default_value (float): Default value if evaluation fails
    ///
    /// Returns:
    ///     float: The evaluated float value
    fn evaluate_float(
        &self,
        flag_key: String,
        context: &Bound<'_, PyDict>,
        default_value: f64,
    ) -> PyResult<f64> {
        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = self.evaluate_optimized(&flag_key, &context_value);

        if result.error_code.is_some() {
            return Ok(default_value);
        }

        match result.value {
            Value::Number(n) => Ok(n.as_f64().unwrap_or(default_value)),
            _ => Ok(default_value),
        }
    }
}

/// Evaluate targeting rules (JSON Logic) against context data.
///
/// This is a helper function for the flagd provider to evaluate targeting rules.
/// For general flag evaluation, use the FlagEvaluator class instead.
///
/// Args:
///     targeting (dict): JSON Logic targeting rules
///     context (dict): Evaluation context data
///
/// Returns:
///     dict: Evaluation result with 'success', 'result', and optional 'error' fields
#[pyfunction]
fn evaluate_targeting(
    py: Python,
    targeting: &Bound<'_, PyDict>,
    context: &Bound<'_, PyDict>,
) -> PyResult<PyObject> {
    use ::flagd_evaluator::operators;

    // Convert Python dicts to JSON values
    let targeting_value: Value = pythonize::depythonize(targeting.as_any()).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Failed to parse targeting: {}", e))
    })?;

    let context_value: Value = pythonize::depythonize(context.as_any()).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Failed to parse context: {}", e))
    })?;

    // Convert to JSON strings for evaluation
    let targeting_str = serde_json::to_string(&targeting_value).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Failed to serialize targeting: {}",
            e
        ))
    })?;

    let context_str = serde_json::to_string(&context_value).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Failed to serialize context: {}",
            e
        ))
    })?;

    // Evaluate using JSON Logic with custom operators
    let logic = operators::create_evaluator();
    let result_dict = PyDict::new_bound(py);

    match logic.evaluate_json(&targeting_str, &context_str) {
        Ok(result) => {
            result_dict.set_item("success", true)?;
            // Convert result back to Python
            let py_result = pythonize::pythonize(py, &result).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Failed to convert result: {}",
                    e
                ))
            })?;
            result_dict.set_item("result", py_result)?;
        }
        Err(e) => {
            result_dict.set_item("success", false)?;
            result_dict.set_item("result", py.None())?;
            result_dict.set_item("error", format!("{}", e))?;
        }
    }

    Ok(result_dict.into())
}

/// flagd_evaluator - Feature flag evaluation
///
/// This module provides native Python bindings for the flagd-evaluator library,
/// offering high-performance feature flag evaluation.
#[pymodule]
fn flagd_evaluator(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<FlagEvaluator>()?;
    m.add_function(wrap_pyfunction!(evaluate_targeting, m)?)?;
    Ok(())
}
