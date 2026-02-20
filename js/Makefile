WASM_SRC = ..
WASM_TARGET = flagd_evaluator.wasm

.PHONY: wasm build test bench clean

wasm:
	cargo build --manifest-path $(WASM_SRC)/Cargo.toml \
		--target wasm32-unknown-unknown --no-default-features --release --lib
	cp $(WASM_SRC)/target/wasm32-unknown-unknown/release/flagd_evaluator.wasm $(WASM_TARGET)

build: wasm
	npx tsc --noEmit

test: wasm
	npx vitest run

bench: wasm
	npx tsx benchmarks/comparison.bench.ts

clean:
	rm -rf node_modules dist $(WASM_TARGET)
