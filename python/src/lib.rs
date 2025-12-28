use ::flagd_evaluator::evaluation::{
    evaluate_bool_flag, evaluate_flag, evaluate_float_flag, evaluate_int_flag, evaluate_string_flag,
};
use ::flagd_evaluator::model::ParsingResult;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

/// FlagEvaluator - Stateful feature flag evaluator
///
/// This class maintains an internal state of feature flag configurations
/// and provides methods to evaluate flags against context data.
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
    state: Option<ParsingResult>,
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
        use ::flagd_evaluator::storage::{set_validation_mode, ValidationMode};

        let mode = if permissive {
            ValidationMode::Permissive
        } else {
            ValidationMode::Strict
        };

        set_validation_mode(mode);

        FlagEvaluator { state: None }
    }

    /// Update the flag configuration state
    ///
    /// Args:
    ///     config (dict): Flag configuration in flagd format
    ///
    /// Returns:
    ///     dict: Update response with changed flag keys
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

        // Parse the configuration
        let parsing_result = ParsingResult::parse(&config_str).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to parse config: {}",
                e
            ))
        })?;

        // Store the state
        self.state = Some(parsing_result.clone());

        // Return update response (simplified - just success)
        let result_dict = PyDict::new_bound(py);
        result_dict.set_item("success", true)?;
        Ok(result_dict.into())
    }

    /// Evaluate a feature flag
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
        let state = self.state.as_ref().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first.",
            )
        })?;

        // Look up the flag
        let flag = state.flags.get(&flag_key).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("Flag not found: {}", flag_key))
        })?;

        // Convert context to JSON Value
        let context_value: Value = pythonize::depythonize(context.as_any())?;

        // Evaluate the flag
        let result = evaluate_flag(flag, &context_value, &state.flag_set_metadata);

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
        let state = self.state.as_ref().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first.",
            )
        })?;

        let flag = state.flags.get(&flag_key).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("Flag not found: {}", flag_key))
        })?;

        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = evaluate_bool_flag(flag, &context_value, &state.flag_set_metadata);

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
        let state = self.state.as_ref().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first.",
            )
        })?;

        let flag = state.flags.get(&flag_key).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("Flag not found: {}", flag_key))
        })?;

        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = evaluate_string_flag(flag, &context_value, &state.flag_set_metadata);

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
        let state = self.state.as_ref().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first.",
            )
        })?;

        let flag = state.flags.get(&flag_key).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("Flag not found: {}", flag_key))
        })?;

        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = evaluate_int_flag(flag, &context_value, &state.flag_set_metadata);

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
        let state = self.state.as_ref().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first.",
            )
        })?;

        let flag = state.flags.get(&flag_key).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("Flag not found: {}", flag_key))
        })?;

        let context_value: Value = pythonize::depythonize(context.as_any())?;
        let result = evaluate_float_flag(flag, &context_value, &state.flag_set_metadata);

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
