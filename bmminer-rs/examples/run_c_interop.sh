#!/bin/bash
#
# Build and run C interoperability test
#
# This script:
# 1. Builds the Rust library
# 2. Compiles the C example
# 3. Runs the C example to verify ABI compatibility
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🦀 BMminer-RS C Interoperability Test${NC}"
echo "======================================"
echo ""

# 1. Build Rust library
echo -e "${YELLOW}Step 1: Building Rust library...${NC}"
RUSTFLAGS="-C target-cpu=native" cargo build --release
echo -e "${GREEN}✓ Rust library built${NC}"
echo ""

# 2. Compile C example
echo -e "${YELLOW}Step 2: Compiling C example...${NC}"
gcc -o c_interop examples/c_interop.c \
    -I./include \
    -L./target/release \
    -lbmminer_rs \
    -lpthread -ldl -lm \
    -O3 \
    -Wall -Wextra

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ C example compiled${NC}"
else
    echo -e "${RED}✗ C compilation failed${NC}"
    exit 1
fi
echo ""

# 3. Run C example
echo -e "${YELLOW}Step 3: Running C interop test...${NC}"
echo ""

LD_LIBRARY_PATH=./target/release ./c_interop

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}======================================"
    echo "✓ All C interop tests passed!"
    echo "======================================${NC}"
else
    echo ""
    echo -e "${RED}======================================"
    echo "✗ C interop tests failed"
    echo "======================================${NC}"
    exit 1
fi

# Cleanup
rm -f ./c_interop
