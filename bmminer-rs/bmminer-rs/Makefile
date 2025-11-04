# Makefile for BMminer-RS C Interoperability
#
# Targets:
#   make all          - Build everything
#   make rust         - Build Rust library
#   make c-example    - Build C example
#   make test         - Run C interop test
#   make bench        - Run all benchmarks
#   make clean        - Clean build artifacts

.PHONY: all rust c-example test bench clean

# Configuration
CC = gcc
CFLAGS = -O3 -Wall -Wextra -I./include
LDFLAGS = -L./target/release -lbmminer_rs -lpthread -ldl -lm
RUSTFLAGS = -C target-cpu=native

# Targets
all: rust c-example

rust:
	@echo "Building Rust library..."
	RUSTFLAGS="$(RUSTFLAGS)" cargo build --release
	@echo "✓ Rust library built"

c-example: rust
	@echo "Building C example..."
	$(CC) $(CFLAGS) -o c_interop examples/c_interop.c $(LDFLAGS)
	@echo "✓ C example built"

test: c-example
	@echo "Running C interop test..."
	@LD_LIBRARY_PATH=./target/release ./c_interop
	@echo "✓ C interop test passed"

bench:
	@echo "Running all benchmarks..."
	@./benchmark_all.sh

bench-native:
	@echo "Running all benchmarks with native CPU optimizations..."
	@./benchmark_all.sh --native

clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -f c_interop
	@echo "✓ Clean complete"

# Help
help:
	@echo "BMminer-RS Makefile"
	@echo ""
	@echo "Usage:"
	@echo "  make all          - Build Rust library and C example"
	@echo "  make rust         - Build Rust library only"
	@echo "  make c-example    - Build C example"
	@echo "  make test         - Run C interoperability test"
	@echo "  make bench        - Run all benchmarks"
	@echo "  make bench-native - Run benchmarks with CPU-specific optimizations"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make help         - Show this help"
