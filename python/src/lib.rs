use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use ::flagd_evaluator::operators;

/// Evaluate a JSON Logic rule against data.
///
/// Args:
///     rule (dict): The JSON Logic rule to evaluate
///     data (dict): The data context for evaluation
///
/// Returns:
///     dict: A result dictionary with keys:
///         - success (bool): Whether evaluation succeeded
///         - result (Any): The evaluation result (if success=True)
///         - error (str): Error message (if success=False)
///
/// Example:
///     >>> result = evaluate_logic({"==": [1, 1]}, {})
///     >>> print(result)
///     {'success': True, 'result': True, 'error': None}
#[pyfunction]
fn evaluate_logic(py: Python, rule: &PyDict, data: &PyDict) -> PyResult<PyObject> {
    // Convert Python dicts to serde_json::Value
    let rule_value: Value = pythonize::depythonize(rule)?;
    let data_value: Value = pythonize::depythonize(data)?;

    // Create evaluator with custom operators
    let logic = operators::create_evaluator();

    // Convert to JSON strings for DataLogic API
    let rule_str = serde_json::to_string(&rule_value)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Failed to serialize rule: {}", e)))?;
    let data_str = serde_json::to_string(&data_value)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Failed to serialize data: {}", e)))?;

    // Evaluate
    match logic.evaluate_json(&rule_str, &data_str) {
        Ok(result) => {
            // Success - convert result back to Python
            let result_dict = PyDict::new(py);
            result_dict.set_item("success", true)?;
            result_dict.set_item("result", pythonize::pythonize(py, &result)?)?;
            result_dict.set_item("error", py.None())?;
            Ok(result_dict.into())
        }
        Err(e) => {
            // Error - return error response
            let result_dict = PyDict::new(py);
            result_dict.set_item("success", false)?;
            result_dict.set_item("result", py.None())?;
            result_dict.set_item("error", format!("{}", e))?;
            Ok(result_dict.into())
        }
    }
}

/// flagd_evaluator - Feature flag evaluation with JSON Logic
///
/// This module provides native Python bindings for the flagd-evaluator library,
/// offering high-performance feature flag evaluation with JSON Logic support.
#[pymodule]
fn flagd_evaluator(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(evaluate_logic, m)?)?;
    Ok(())
}
