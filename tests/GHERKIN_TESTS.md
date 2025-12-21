# Gherkin Test Suite for flagd-evaluator

This directory contains a Cucumber/Gherkin test suite that runs the official flagd testbed scenarios against the flagd-evaluator.

## Overview

The Gherkin tests (`gherkin_tests.rs`) execute feature files from `testbed/gherkin/` to ensure the evaluator conforms to the flagd specification.

## Test Structure

### Feature Files Tested

1. **evaluation.feature** - Basic flag evaluation and resolution
2. **targeting.feature** - Targeting rules with custom operators (fractional, sem_ver, starts_with, ends_with)
3. **contextEnrichment.feature** - Context enrichment ($flagd properties)
4. **metadata.feature** - Flag and flag-set metadata merging

### Flag Configurations

Test data is loaded from `testbed/flags/`:
- `testing-flags.json` - Basic flag types
- `zero-flags.json` - Zero-value flags
- `custom-ops.json` - Custom operator flags (fractional, sem_ver, etc.)
- `evaluator-refs.json` - $evaluators and $ref resolution
- `edge-case-flags.json` - Edge cases and error handling
- `metadata-flags.json` - Metadata testing

## Running the Tests

```bash
# Run all Gherkin tests
cargo test --test gherkin_tests

# Run specific test suite
cargo test --test gherkin_tests run_evaluation_tests
cargo test --test gherkin_tests run_targeting_tests
cargo test --test gherkin_tests run_metadata_tests
cargo test --test gherkin_tests run_context_enrichment_tests
```

## Implementation

### Step Definitions

The test implements standard Gherkin step definitions:

**Given steps:**
- `Given a stable flagd provider` - Loads and merges all flag configurations
- `Given a {type}-flag with key "{key}" and a default value "{default}"` - Sets up flag for evaluation
- `Given a context containing a key "{key}", with type "{type}" and with value "{value}"` - Adds context data
- `Given a context containing a nested property...` - Adds nested context
- `Given a context containing a targeting key with value "{value}"` - Sets targeting key

**When steps:**
- `When the flag was evaluated with details` - Evaluates the flag

**Then steps:**
- `Then the resolved details value should be "{value}"` - Asserts result value
- `Then the reason should be "{reason}"` - Asserts resolution reason
- `Then the error-code should be "{code}"` - Asserts error code
- `Then the resolved metadata should contain` - Asserts metadata values
- `Then the resolved metadata is empty` - Asserts no metadata

### Filtering

Tests are filtered to skip scenarios that require:
- RPC communication (`@rpc` tag)
- Connection management (`@grace` tag)
- Caching (`@caching` tag)

Only `@in-process` and `@file` scenarios are executed, which are relevant for the evaluator.

## Current Status

### Working
✅ Test infrastructure setup
✅ Feature file parsing
✅ Step definition implementation
✅ Flag configuration merging
✅ Basic evaluation flow
✅ Context building

### Known Issues

⚠️ **Thread-local storage limitation**: The flagd-evaluator uses thread-local storage for flag state, which doesn't work well with Cucumber's async test execution model. Each scenario may run in a different async task/thread, causing "No flag state loaded" errors.

**Potential solutions:**
1. Modify the evaluator to use Arc<RwLock<>> instead of thread-local storage for tests
2. Implement a test-specific evaluator that doesn't use thread-local storage
3. Use a synchronous test runner instead of async
4. Add a feature flag to the evaluator for test mode that uses global state

## Dependencies

```toml
[dev-dependencies]
cucumber = "0.21"
tokio = { version = "1.0", features = ["macros", "rt-multi-thread"] }
async-trait = "0.1"
glob = "0.3"
```

## Example Output

```
Feature: flagd evaluations
  Scenario Outline: Resolve values
   ✔  Given a Boolean-flag with key "boolean-flag" and a default value "false"
   ✔  When the flag was evaluated with details
   ✔  Then the resolved details value should be "true"

[Summary]
1 feature
4 scenarios (3 passed, 1 failed)
18 steps (16 passed, 2 failed)
```

## Future Enhancements

- [ ] Fix thread-local storage issues for async tests
- [ ] Add more detailed error reporting
- [ ] Support for additional feature files (config.feature, events.feature)
- [ ] Performance benchmarking using Gherkin scenarios
- [ ] CI integration for continuous spec compliance checking
