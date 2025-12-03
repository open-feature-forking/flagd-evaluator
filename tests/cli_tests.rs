//! CLI integration tests for flagd-eval binary.
//!
//! These tests verify the CLI commands work correctly end-to-end.

use assert_cmd::Command;
use predicates::prelude::*;

/// Get the path to the CLI binary.
fn cmd() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("flagd-eval").expect("Failed to find flagd-eval binary")
}

// ============================================================================
// Help and Version Tests
// ============================================================================

#[test]
fn test_help() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("flagd-eval"))
        .stdout(predicate::str::contains("eval"))
        .stdout(predicate::str::contains("test"))
        .stdout(predicate::str::contains("operators"));
}

#[test]
fn test_version() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("flagd-eval"));
}

// ============================================================================
// Eval Command Tests
// ============================================================================

#[test]
fn test_eval_inline_json() {
    cmd()
        .args(["eval", "--rule", r#"{"==": [1, 1]}"#, "--data", "{}"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Evaluation succeeded"))
        .stdout(predicate::str::contains("true"));
}

#[test]
fn test_eval_comparison() {
    cmd()
        .args([
            "eval",
            "--rule",
            r#"{">": [{"var": "age"}, 18]}"#,
            "--data",
            r#"{"age": 25}"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("true"));
}

#[test]
fn test_eval_fractional() {
    cmd()
        .args([
            "eval",
            "--rule",
            r#"{"fractional": ["user-123", ["control", 50, "treatment", 50]]}"#,
            "--data",
            "{}",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Evaluation succeeded"));
}

#[test]
fn test_eval_file_rule() {
    cmd()
        .args([
            "eval",
            "--rule",
            "@examples/rules/basic.json",
            "--data",
            r#"{"age": 25}"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("adult"));
}

#[test]
fn test_eval_file_data() {
    cmd()
        .args([
            "eval",
            "--rule",
            r#"{"var": "enabled"}"#,
            "--data",
            "@examples/data/simple.json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("true"));
}

#[test]
fn test_eval_both_files() {
    cmd()
        .args([
            "eval",
            "--rule",
            "@examples/rules/fractional.json",
            "--data",
            "@examples/data/app-v2.json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Evaluation succeeded"));
}

#[test]
fn test_eval_pretty() {
    cmd()
        .args([
            "eval",
            "--rule",
            r#"{"var": "data"}"#,
            "--data",
            r#"{"data": {"nested": 42}}"#,
            "--pretty",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("nested"));
}

#[test]
fn test_eval_invalid_json() {
    cmd()
        .args(["eval", "--rule", "not valid json", "--data", "{}"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse JSON"));
}

#[test]
fn test_eval_missing_file() {
    cmd()
        .args(["eval", "--rule", "@nonexistent.json", "--data", "{}"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("File not found"));
}

#[test]
fn test_eval_fractional_missing_var() {
    // In datalogic-rs v3, missing variables return null (per JSON Logic spec),
    // which is converted to an empty string and successfully hashed.
    // This is correct behavior - no error should be thrown.
    cmd()
        .args([
            "eval",
            "--rule",
            r#"{"fractional": [{"var": "missing"}, ["a", 50, "b", 50]]}"#,
            "--data",
            "{}",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Evaluation succeeded"));
}

// ============================================================================
// Test Command Tests
// ============================================================================

#[test]
fn test_test_suite() {
    cmd()
        .args(["test", "examples/rules/test-suite.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Example Test Suite"))
        .stdout(predicate::str::contains("passed"))
        .stdout(predicate::str::contains("0 failed"));
}

#[test]
fn test_test_verbose() {
    cmd()
        .args(["test", "examples/rules/test-suite.json", "--verbose"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Rule:"))
        .stdout(predicate::str::contains("Data:"))
        .stdout(predicate::str::contains("Expected:"));
}

#[test]
fn test_test_missing_file() {
    cmd()
        .args(["test", "nonexistent.json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Test file not found"));
}

#[test]
fn test_test_invalid_format() {
    // Create a temporary invalid test file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("invalid-test.json");
    std::fs::write(&temp_file, "not valid json").unwrap();

    cmd()
        .args(["test", temp_file.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse test suite"));

    std::fs::remove_file(temp_file).ok();
}

// ============================================================================
// Operators Command Tests
// ============================================================================

#[test]
fn test_operators() {
    cmd()
        .args(["operators"])
        .assert()
        .success()
        .stdout(predicate::str::contains("fractional"))
        .stdout(predicate::str::contains(
            "Percentage-based bucket assignment",
        ))
        .stdout(predicate::str::contains("MurmurHash3"));
}

#[test]
fn test_operators_shows_syntax() {
    cmd()
        .args(["operators"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"fractional":"#));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_eval_empty_data() {
    cmd()
        .args(["eval", "--rule", r#"{"==": [1, 1]}"#, "--data", "{}"])
        .assert()
        .success();
}

#[test]
fn test_eval_complex_nested_rule() {
    let rule = r#"{"if": [{"and": [{">=": [{"var": "age"}, 18]}, {"<": [{"var": "age"}, 65]}]}, "working age", "not working age"]}"#;
    cmd()
        .args(["eval", "--rule", rule, "--data", r#"{"age": 30}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("working age"));
}

#[test]
fn test_eval_unicode() {
    cmd()
        .args([
            "eval",
            "--rule",
            r#"{"var": "greeting"}"#,
            "--data",
            r#"{"greeting": "こんにちは"}"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("こんにちは"));
}
