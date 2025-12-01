//! CLI binary for testing and debugging JSON Logic rules.
//!
//! This binary provides a command-line interface for evaluating JSON Logic rules
//! without requiring WASM compilation or Java integration.
//!
//! # Commands
//!
//! - `eval`: Evaluate a single rule against data
//! - `test`: Run a test suite from a JSON file
//! - `operators`: List available custom operators

use clap::{Parser, Subcommand};
use colored::Colorize;
use flagd_evaluator::EvaluationResponse;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Instant;

/// CLI for testing and debugging flagd JSON Logic rules
#[derive(Parser)]
#[command(name = "flagd-eval")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Evaluate a JSON Logic rule against data
    Eval {
        /// JSON Logic rule (inline JSON or @file.json)
        #[arg(short, long)]
        rule: String,

        /// Data to evaluate against (inline JSON or @file.json)
        #[arg(short, long)]
        data: String,

        /// Pretty-print the output
        #[arg(short, long)]
        pretty: bool,
    },

    /// Run a test suite from a JSON file
    Test {
        /// Path to the test suite JSON file
        test_file: String,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// List available custom operators
    Operators,
}

/// Test case definition for test suites
#[derive(Debug, Deserialize)]
struct TestCase {
    description: String,
    rule: Value,
    data: Value,
    expected: Value,
}

/// Test suite definition
#[derive(Debug, Deserialize)]
struct TestSuite {
    name: String,
    tests: Vec<TestCase>,
}

/// Load content from a file or parse as inline JSON.
/// File references use the `@` prefix (e.g., `@examples/rules/basic.json`).
fn load_json(input: &str) -> Result<Value, String> {
    if let Some(file_path) = input.strip_prefix('@') {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(format!("File not found: {}", file_path));
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file '{}': {}", file_path, e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse JSON from '{}': {}", file_path, e))
    } else {
        serde_json::from_str(input).map_err(|e| format!("Failed to parse JSON: {}", e))
    }
}

/// Evaluate a JSON Logic rule against data using the library's internal logic.
fn evaluate_rule(rule: &Value, data: &Value) -> EvaluationResponse {
    let rule_str = rule.to_string();
    let data_str = data.to_string();

    // Check for custom fractional operator first
    if let Some(result) = handle_fractional(rule, data) {
        return match result {
            Ok(value) => EvaluationResponse::success(value),
            Err(e) => EvaluationResponse::error(e),
        };
    }

    // Use datalogic-rs for standard JSON Logic evaluation
    let logic = datalogic_rs::DataLogic::new();
    match logic.evaluate_json(&rule_str, &data_str) {
        Ok(result) => EvaluationResponse::success(result),
        Err(e) => EvaluationResponse::error(format!("Evaluation error: {}", e)),
    }
}

/// Handle the custom fractional operator if present in the rule.
fn handle_fractional(rule: &Value, data: &Value) -> Option<Result<Value, String>> {
    let args = rule.get("fractional")?;

    let args_array = match args {
        Value::Array(arr) if arr.len() >= 2 => arr,
        _ => {
            return Some(Err(
                "fractional operator requires an array with at least 2 elements".to_string(),
            ))
        }
    };

    // First argument is the bucket key (can be a value or a var reference)
    let bucket_key = match &args_array[0] {
        Value::String(s) => s.clone(),
        Value::Object(obj) if obj.contains_key("var") => {
            let var_path = match obj.get("var") {
                Some(Value::String(s)) => s,
                _ => return Some(Err("var reference must be a string".to_string())),
            };

            let mut current = data;
            for part in var_path.split('.') {
                current = match current.get(part) {
                    Some(v) => v,
                    None => return Some(Err(format!("Variable '{}' not found in data", var_path))),
                };
            }

            match current {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => {
                    return Some(Err(format!(
                        "Variable '{}' must be a string or number",
                        var_path
                    )))
                }
            }
        }
        Value::Number(n) => n.to_string(),
        _ => {
            return Some(Err(
                "First argument must be a string, number, or var reference".to_string(),
            ))
        }
    };

    // Second argument is the buckets array
    let buckets = match &args_array[1] {
        Value::Array(arr) => arr.as_slice(),
        _ => {
            return Some(Err(
                "Second argument must be an array of bucket definitions".to_string(),
            ))
        }
    };

    match flagd_evaluator::fractional(&bucket_key, buckets) {
        Ok(bucket_name) => Some(Ok(Value::String(bucket_name))),
        Err(e) => Some(Err(e)),
    }
}

/// Run the eval command.
fn run_eval(rule: &str, data: &str, pretty: bool) -> Result<(), String> {
    let rule_value = load_json(rule)?;
    let data_value = load_json(data)?;

    let start = Instant::now();
    let response = evaluate_rule(&rule_value, &data_value);
    let duration = start.elapsed();

    if response.success {
        println!("{} Evaluation succeeded", "✓".green());
        let result = response.result.unwrap_or(Value::Null);
        if pretty {
            println!(
                "Result: {}",
                serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| result.to_string())
                    .green()
            );
        } else {
            println!("Result: {}", result.to_string().green());
        }
        println!("Time: {:?}", duration);
        Ok(())
    } else {
        let error_msg = response
            .error
            .unwrap_or_else(|| "Unknown error".to_string());
        println!("{} Evaluation failed", "✗".red());
        println!("Error: {}", error_msg.red());
        Err(error_msg)
    }
}

/// Run the test command.
fn run_test(test_file: &str, verbose: bool) -> Result<(), String> {
    let path = Path::new(test_file);
    if !path.exists() {
        return Err(format!("Test file not found: {}", test_file));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read test file '{}': {}", test_file, e))?;

    let suite: TestSuite = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse test suite '{}': {}", test_file, e))?;

    println!("Running: {}", suite.name.bold());
    println!();

    let mut passed = 0;
    let mut failed = 0;
    let mut total_duration = std::time::Duration::ZERO;

    for test in &suite.tests {
        let start = Instant::now();
        let response = evaluate_rule(&test.rule, &test.data);
        let duration = start.elapsed();
        total_duration += duration;

        let actual = response.result.clone().unwrap_or(Value::Null);
        let success = response.success && actual == test.expected;

        if success {
            passed += 1;
            println!("{} {} ({:?})", "✓".green(), test.description, duration);
            if verbose {
                println!(
                    "  Rule: {}",
                    serde_json::to_string(&test.rule).unwrap_or_default()
                );
                println!(
                    "  Data: {}",
                    serde_json::to_string(&test.data).unwrap_or_default()
                );
                println!(
                    "  Expected: {}",
                    serde_json::to_string(&test.expected).unwrap_or_default()
                );
                println!(
                    "  Actual: {}",
                    serde_json::to_string(&actual).unwrap_or_default()
                );
                println!();
            }
        } else {
            failed += 1;
            println!("{} {} ({:?})", "✗".red(), test.description, duration);
            if !response.success {
                println!(
                    "  Error: {}",
                    response
                        .error
                        .unwrap_or_else(|| "Unknown error".to_string())
                        .red()
                );
            } else {
                println!(
                    "  Expected: {}",
                    serde_json::to_string(&test.expected)
                        .unwrap_or_default()
                        .green()
                );
                println!(
                    "  Actual: {}",
                    serde_json::to_string(&actual).unwrap_or_default().red()
                );
            }
            println!();
        }
    }

    println!();
    let summary = format!(
        "Results: {} passed, {} failed ({:?})",
        passed, failed, total_duration
    );
    if failed == 0 {
        println!("{}", summary.green());
        Ok(())
    } else {
        println!("{}", summary.red());
        Err(format!("{} test(s) failed", failed))
    }
}

/// Run the operators command.
fn run_operators() {
    println!("{}", "Available Custom Operators:".bold());
    println!();

    println!("• {}", "fractional".cyan());
    println!("  Percentage-based bucket assignment for A/B testing");
    println!(
        "  {}",
        r#"{"fractional": [{"var": "key"}, ["a", 50, "b", 50]]}"#.dimmed()
    );
    println!();
    println!("  Properties:");
    println!("  - Consistent: Same bucket key always returns the same bucket");
    println!("  - Deterministic: Results are reproducible across invocations");
    println!("  - Uses MurmurHash3 for uniform distribution");
    println!();

    println!("{}", "Standard JSON Logic Operators:".bold());
    println!();
    println!("  All standard JSON Logic operators are supported via datalogic-rs.");
    println!("  See https://jsonlogic.com/operations.html for the full list.");
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Eval { rule, data, pretty } => run_eval(&rule, &data, pretty),
        Commands::Test { test_file, verbose } => run_test(&test_file, verbose),
        Commands::Operators => {
            run_operators();
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
