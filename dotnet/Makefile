WASM_SRC := ../target/wasm32-unknown-unknown/release/flagd_evaluator.wasm
WASM_DST := src/FlagdEvaluator/flagd_evaluator.wasm

.PHONY: wasm build test bench clean

wasm:
	cargo build --manifest-path ../Cargo.toml --target wasm32-unknown-unknown --no-default-features --release --lib
	cp $(WASM_SRC) $(WASM_DST)

build:
	dotnet build dotnet.sln -c Release

test:
	dotnet test dotnet.sln -c Release --logger "console;verbosity=detailed"

bench:
	dotnet run --project benchmarks/FlagdEvaluator.Benchmarks/FlagdEvaluator.Benchmarks.csproj -c Release

clean:
	dotnet clean dotnet.sln
	rm -rf src/FlagdEvaluator/bin src/FlagdEvaluator/obj
	rm -rf tests/FlagdEvaluator.Tests/bin tests/FlagdEvaluator.Tests/obj
	rm -rf benchmarks/FlagdEvaluator.Benchmarks/bin benchmarks/FlagdEvaluator.Benchmarks/obj
