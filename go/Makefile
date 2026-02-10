.PHONY: build wasm test bench bench-comparison fmt lint clean

# Build the WASM binary from the Rust source
wasm:
	cd .. && cargo build --target wasm32-unknown-unknown --no-default-features --release --lib
	cp ../target/wasm32-unknown-unknown/release/flagd_evaluator.wasm .

# Run tests
test:
	go test -v -count=1 ./...

# Run benchmarks
bench:
	go test -bench=. -benchmem -count=3 ./...

# Run comparison benchmarks (requires diegoholiveira/jsonlogic/v3)
bench-comparison:
	go test -tags comparison -bench=. -benchmem -count=3 ./...

# Format code
fmt:
	go fmt ./...

# Lint code
lint:
	go vet ./...

# Clean build artifacts
clean:
	go clean -testcache
