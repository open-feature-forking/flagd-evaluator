# Python Native Bindings Implementation TODO

## Day 1: Workspace Setup + Basic evaluate_logic
- [x] Convert root Cargo.toml to workspace
- [x] Create python/ directory structure
- [x] Add python/Cargo.toml with PyO3 dependencies
- [x] Add python/pyproject.toml with maturin config
- [x] Implement basic evaluate_logic function in python/src/lib.rs
- [ ] Test local build with `maturin develop` (skipped - cargo build works)

## Day 2: FlagEvaluator Class + State Management
- [x] Implement FlagEvaluator PyClass
- [x] Add __init__ method
- [x] Implement update_state method
- [x] Implement evaluate method
- [x] Implement type-specific methods (evaluate_bool, evaluate_string, evaluate_int, evaluate_float)
- [ ] Test state management (deferred to Day 3)

## Day 3: Python Tests + Type Stubs
- [ ] Create python/tests/test_basic.py
- [ ] Create python/tests/test_operators.py
- [ ] Create python/tests/test_flag_evaluation.py
- [ ] Add python/flagd_evaluator.pyi type stub file
- [ ] Run all tests locally

## Day 4: CI/CD Pipeline + Wheel Builds
- [ ] Create .github/workflows/python-wheels.yml
- [ ] Configure maturin-action for multi-platform builds
- [ ] Test wheel builds locally
- [ ] Update .github/workflows/ci.yml to test Python bindings

## Day 5: Documentation + Examples + Benchmarks
- [ ] Create python/README.md
- [ ] Create python/examples/basic_usage.py
- [ ] Create python/examples/flag_evaluation.py
- [ ] Create python/examples/custom_operators.py
- [ ] Create python/benchmarks/bench_vs_wasm.py
- [ ] Update main README.md with native bindings section
- [ ] Update CLAUDE.md

## Progress Tracking
- Current Day: Day 3 (Python Tests + Type Stubs)
- Last Completed: Day 2 (FlagEvaluator Class + State Management)
- Blocked On: None
