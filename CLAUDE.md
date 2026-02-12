# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

flagd-evaluator is a **Rust-based feature flag evaluation engine** that replaces per-language JSON Logic implementations (json-logic-java, json-logic-utils, etc.) with a single core — one implementation, one test suite, consistent behavior everywhere. Thin wrapper libraries expose it via WASM runtimes (Java/Chicory, Go/wazero, JS, .NET) or native bindings (Python/PyO3). The best integration strategy is chosen per language based on benchmarks — e.g., Python benchmarks showed PyO3 native bindings outperform WASM (wasmtime-py), while Go and Java perform well with their WASM runtimes. See [BENCHMARKS.md](BENCHMARKS.md) for the full comparison matrix.

## Quick Reference

```bash
cargo build                                                              # dev build
cargo build --target wasm32-unknown-unknown --no-default-features --release --lib  # WASM build
cargo test                                                               # all tests
cargo fmt && cargo clippy -- -D warnings                                 # lint (required before commit)
cd python && uv sync --group dev && maturin develop && pytest tests/ -v  # python bindings
```

## Architecture at a Glance

```
src/
├── lib.rs          # WASM exports (update_state, evaluate, alloc, dealloc)
├── evaluation.rs   # Flag evaluation logic, context enrichment ($flagd properties)
├── memory.rs       # WASM memory management, pointer packing
├── storage/        # Thread-local flag state storage
├── operators/      # Custom operators: fractional, sem_ver, starts_with, ends_with
├── model/          # Flag configuration data structures
└── validation.rs   # JSON Schema validation (boon crate)
```

**Key concepts:**
- **Packed u64 returns** — All WASM exports return upper 32 bits = pointer, lower 32 bits = length
- **Thread-local storage** — Flag state stored per-thread; `update_state` detects changed flags
- **Context enrichment** — `$flagd.flagKey`, `$flagd.timestamp`, and `targetingKey` auto-injected

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design, memory model, error handling, and cross-language integration patterns.

## Development Workflow

### Issue First

Always create a GitHub issue before starting work. This ensures traceability and clear scope.

```bash
gh issue create --title "feat(go): add Go WASM bindings" --body "Description of the work"
```

### Work in Worktrees

All feature work happens in git worktrees under `./worktrees/`. This keeps the main working directory clean and allows parallel work on multiple issues.

```bash
# Create a branch and worktree for the issue
git worktree add worktrees/<short-name> -b feat/<short-name>

# Example for issue #42
git worktree add worktrees/go-bindings -b feat/go-bindings

# Work inside the worktree
cd worktrees/go-bindings
```

Branch naming should match the issue scope (e.g., `feat/go-bindings`, `fix/memory-leak`, `refactor/storage`).

### Plan Before Implementing

Before writing any code for an issue, **always enter planning mode** first. This ensures the approach is sound before investing effort.

- Use `EnterPlanMode` to explore the codebase, understand existing patterns, and design the implementation
- Present the plan for approval before writing code
- Use `AskUserQuestion` during planning to clarify ambiguous requirements

### Use Sub-Agents

Leverage the `Task` tool with sub-agents liberally:

- **Explore agents** for codebase research and understanding existing patterns
- **Plan agents** for designing implementation approaches
- **Bash agents** for running builds, tests, and git operations
- **General-purpose agents** for multi-step research tasks

Launch multiple agents **in parallel** when their work is independent (e.g., researching two different subsystems simultaneously). This maximizes throughput.

### Workflow Summary

1. **Create a GitHub issue** describing the work
2. **Create a worktree** under `./worktrees/` on a feature branch
3. **Enter planning mode** to explore and design the approach
4. **Get plan approval** before writing code
5. **Implement** with regular commits referencing the issue
6. **Run tests** before creating a PR
7. **Create a PR** linking back to the issue

## Key Rules

**Memory safety (WASM exports):**
- Never panic — return JSON error responses
- Always validate UTF-8 via `string_from_memory()`
- All `unsafe` blocks require `// SAFETY:` comments
- Build WASM with `--no-default-features`

**Testing:**
- Run tests after behavior changes and before PRs
- Don't run tests during exploration or documentation reading
- Integration tests: `tests/integration_tests.rs`; Gherkin tests: `tests/gherkin_tests.rs`

**Commits:**
- Follow [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <description>`
- Commit regularly after logical units of work
- See CONTRIBUTING.md for full commit and PR guidelines

## Deep-Dive References

| Topic | File |
|-------|------|
| Architecture, memory model, cross-language integration | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Build commands, code style, commit conventions, PR process | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Benchmark matrix, performance expectations, scale testing | [BENCHMARKS.md](BENCHMARKS.md) |
| Python bindings (PyO3), building, testing, CI/CD | [python/README.md](python/README.md) |
| Java library, Chicory integration | [java/README.md](java/README.md) |
| API reference, usage examples, custom operators | [README.md](README.md) |
| Host function requirements (timestamp, random) | [HOST_FUNCTIONS.md](HOST_FUNCTIONS.md) |

## External Specifications

- [flagd Provider Specification](https://github.com/open-feature/flagd/blob/main/docs/reference/specifications/providers.md)
- [flagd Custom Operations](https://flagd.dev/reference/specifications/custom-operations/)
- [Flag Definitions Schema](https://flagd.dev/reference/flag-definitions/)
- [JSON Logic](https://jsonlogic.com/)
- [datalogic-rs](https://github.com/cozylogic/datalogic-rs)
- [Chicory WASM Runtime](https://github.com/nicknisi/chicory)
