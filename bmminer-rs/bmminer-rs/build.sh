#!/bin/bash
#
# Build script for bmminer-rs
#
# Usage:
#   ./build.sh [debug|release|arm|bench|test]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🦀 BMminer-RS Build Script${NC}"
echo "=================================="

# Detect command
CMD="${1:-release}"

case "$CMD" in
    debug)
        echo -e "${YELLOW}Building debug version...${NC}"
        cargo build
        echo -e "${GREEN}✓ Debug build complete${NC}"
        echo "Binary: target/debug/libbmminer_rs.rlib"
        ;;

    release)
        echo -e "${YELLOW}Building release version with maximum optimizations...${NC}"
        RUSTFLAGS="-C target-cpu=native -C link-arg=-s" \
            cargo build --release

        echo -e "${GREEN}✓ Release build complete${NC}"
        echo "Binary: target/release/libbmminer_rs.rlib"

        # Show binary size
        SIZE=$(ls -lh target/release/libbmminer_rs.rlib | awk '{print $5}')
        echo "Size: $SIZE"
        ;;

    arm)
        echo -e "${YELLOW}Cross-compiling for ARM (Cortex-A53)...${NC}"

        # Check if cross-compiler is installed
        if ! command -v aarch64-linux-gnu-gcc &> /dev/null; then
            echo -e "${RED}Error: aarch64-linux-gnu-gcc not found${NC}"
            echo "Install with: sudo apt-get install gcc-aarch64-linux-gnu"
            exit 1
        fi

        # Set cross-compilation environment
        export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
        export AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

        # Build with ARM-specific optimizations
        RUSTFLAGS="\
            -C target-cpu=cortex-a53 \
            -C link-arg=-s \
            -C prefer-dynamic=no \
            -C target-feature=+neon,+fp-armv8" \
        cargo build --release --target aarch64-unknown-linux-gnu

        echo -e "${GREEN}✓ ARM build complete${NC}"
        echo "Binary: target/aarch64-unknown-linux-gnu/release/libbmminer_rs.rlib"

        # Show binary size
        SIZE=$(ls -lh target/aarch64-unknown-linux-gnu/release/libbmminer_rs.rlib 2>/dev/null | awk '{print $5}' || echo "N/A")
        echo "Size: $SIZE"
        ;;

    bench)
        echo -e "${YELLOW}Running benchmarks...${NC}"
        echo ""
        echo "This will take a few minutes. Results will be saved to:"
        echo "  target/criterion/*/report/index.html"
        echo ""

        cargo bench

        echo ""
        echo -e "${GREEN}✓ Benchmarks complete${NC}"
        echo "View HTML reports in: target/criterion/"
        ;;

    test)
        echo -e "${YELLOW}Running tests...${NC}"

        # Run unit tests
        echo "Running unit tests..."
        cargo test

        # Run doc tests
        echo "Running doc tests..."
        cargo test --doc

        echo -e "${GREEN}✓ All tests passed${NC}"
        ;;

    clean)
        echo -e "${YELLOW}Cleaning build artifacts...${NC}"
        cargo clean
        echo -e "${GREEN}✓ Clean complete${NC}"
        ;;

    *)
        echo -e "${RED}Unknown command: $CMD${NC}"
        echo ""
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  debug     - Build debug version (default)"
        echo "  release   - Build release version with optimizations"
        echo "  arm       - Cross-compile for ARM (Bitmain hardware)"
        echo "  bench     - Run performance benchmarks"
        echo "  test      - Run all tests"
        echo "  clean     - Remove build artifacts"
        exit 1
        ;;
esac

echo "=================================="
