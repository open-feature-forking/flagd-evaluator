use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use ::flagd_evaluator::model::ParsingResult;
use ::flagd_evaluator::evaluation::{evaluate_flag, evaluate_bool_flag, evaluate_string_flag, evaluate_int_flag, evaluate_float_flag};

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
    #[new]
    fn new() -> Self {
        FlagEvaluator { state: None }
    }

    /// Update the flag configuration state
    ///
    /// Args:
    ///     config (dict): Flag configuration in flagd format
    ///
    /// Returns:
    ///     dict: Update response with changed flag keys
    fn update_state(&mut self, py: Python, config: &PyDict) -> PyResult<PyObject> {
        // Convert Python dict to JSON Value
        let config_value: Value = pythonize::depythonize(config)?;

        // Convert to JSON string for parsing
        let config_str = serde_json::to_string(&config_value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Failed to serialize config: {}", e)
            ))?;

        // Parse the configuration
        let parsing_result = ParsingResult::parse(&config_str)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Failed to parse config: {}", e)
            ))?;

        // Store the state
        self.state = Some(parsing_result.clone());

        // Return update response (simplified - just success)
        let result_dict = PyDict::new(py);
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
    fn evaluate(&self, py: Python, flag_key: String, context: &PyDict) -> PyResult<PyObject> {
        let state = self.state.as_ref()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first."
            ))?;

        // Look up the flag
        let flag = state.flags.get(&flag_key)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>(
                format!("Flag not found: {}", flag_key)
            ))?;

        // Convert context to JSON Value
        let context_value: Value = pythonize::depythonize(context)?;

        // Evaluate the flag
        let result = evaluate_flag(flag, &context_value, &state.flag_set_metadata);

        // Convert result to Python dict
        pythonize::pythonize(py, &result)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Failed to convert result: {}", e)))
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
    fn evaluate_bool(&self, flag_key: String, context: &PyDict, default_value: bool) -> PyResult<bool> {
        let state = self.state.as_ref()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first."
            ))?;

        let flag = state.flags.get(&flag_key)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>(
                format!("Flag not found: {}", flag_key)
            ))?;

        let context_value: Value = pythonize::depythonize(context)?;
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
    fn evaluate_string(&self, flag_key: String, context: &PyDict, default_value: String) -> PyResult<String> {
        let state = self.state.as_ref()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first."
            ))?;

        let flag = state.flags.get(&flag_key)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>(
                format!("Flag not found: {}", flag_key)
            ))?;

        let context_value: Value = pythonize::depythonize(context)?;
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
    fn evaluate_int(&self, flag_key: String, context: &PyDict, default_value: i64) -> PyResult<i64> {
        let state = self.state.as_ref()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first."
            ))?;

        let flag = state.flags.get(&flag_key)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>(
                format!("Flag not found: {}", flag_key)
            ))?;

        let context_value: Value = pythonize::depythonize(context)?;
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
    fn evaluate_float(&self, flag_key: String, context: &PyDict, default_value: f64) -> PyResult<f64> {
        let state = self.state.as_ref()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No state loaded. Call update_state() first."
            ))?;

        let flag = state.flags.get(&flag_key)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>(
                format!("Flag not found: {}", flag_key)
            ))?;

        let context_value: Value = pythonize::depythonize(context)?;
        let result = evaluate_float_flag(flag, &context_value, &state.flag_set_metadata);

        match result.value {
            Value::Number(n) => Ok(n.as_f64().unwrap_or(default_value)),
            _ => Ok(default_value),
        }
    }
}

/// flagd_evaluator - Feature flag evaluation
///
/// This module provides native Python bindings for the flagd-evaluator library,
/// offering high-performance feature flag evaluation.
#[pymodule]
fn flagd_evaluator(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<FlagEvaluator>()?;
    Ok(())
}
