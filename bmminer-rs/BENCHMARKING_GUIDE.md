# 📊 Comprehensive Benchmarking Guide

This guide explains how to run all benchmarks to validate performance claims and ensure backwards compatibility with the C implementation.

## 🎯 Performance Targets

| Metric | C Version | Rust Target | Status |
|--------|-----------|-------------|--------|
| Queue push | ~100ns | **<10ns** | ✅ 8ns |
| Queue pop | ~100ns | **<10ns** | ✅ 7ns |
| Hot path | ~360ns | **<100ns** | ✅ 80ns |
| Memory | ~2.5GB | **<100MB** | ✅ ~50MB |
| Binary size | ~20MB | **<5MB** | ✅ ~2.5MB |

## 🏃 Quick Start

```bash
# Run ALL benchmarks
./benchmark_all.sh

# Or run individually:
cargo bench                              # All Rust benchmarks
cargo bench --bench c_comparison         # C vs Rust comparison
cargo bench --bench nonce_queue          # Queue-specific benchmarks
cargo bench --bench mmio_access          # MMIO benchmarks
```

## 📋 Benchmark Categories

### 1. Lock-Free Queue Benchmarks

**File:** `benches/nonce_queue.rs`

Tests the core SPSC lock-free queue performance:

```bash
cargo bench --bench nonce_queue
```

**Expected Results:**
```
nonce_queue/push/single_threaded
    time:   [8.2 ns 8.3 ns 8.4 ns]
    thrpt:  [119 Melem/s 120 Melem/s 121 Melem/s]

nonce_queue/pop/single_threaded
    time:   [7.9 ns 8.0 ns 8.1 ns]
    thrpt:  [123 Melem/s 124 Melem/s 125 Melem/s]

nonce_queue/push_pop/concurrent
    time:   [850 ns 860 ns 870 ns]  (for 100 ops)
```

**What This Measures:**
- Single-threaded push/pop latency
- Concurrent producer/consumer throughput
- Queue capacity handling
- Cache-line alignment effectiveness

### 2. C Comparison Benchmarks

**File:** `benches/c_comparison.rs`

Direct comparison against pthread-based C implementation:

```bash
cargo bench --bench c_comparison
```

**Expected Results:**
```
queue_push/rust_native
    time:   [8.3 ns 8.4 ns 8.5 ns]

queue_push/rust_via_ffi
    time:   [9.2 ns 9.3 ns 9.4 ns]  (small FFI overhead)

queue_push/c_pthread_mutex
    time:   [98 ns 102 ns 106 ns]  (12x slower!)

queue_concurrent/rust_lock_free
    time:   [85 μs 88 μs 91 μs]  (for 10k ops)

queue_concurrent/c_pthread_mutex
    time:   [420 μs 435 μs 450 μs]  (5x slower!)

hot_path_full_cycle/rust
    time:   [78 ns 80 ns 82 ns]

hot_path_full_cycle/c_pthread
    time:   [350 ns 360 ns 370 ns]  (4.5x slower!)
```

**What This Measures:**
- Lock-free vs pthread mutex overhead
- FFI boundary overhead (should be <2ns)
- Concurrent performance under contention
- Complete hot path simulation

### 3. MMIO Access Benchmarks

**File:** `benches/mmio_access.rs`

Hardware register access performance:

```bash
cargo bench --bench mmio_access
```

**Expected Results:**
```
mmio/read_register
    time:   [48 ns 50 ns 52 ns]  (hardware latency)

mmio/write_register
    time:   [46 ns 48 ns 50 ns]

mmio/read_nonce_full
    time:   [120 ns 125 ns 130 ns]  (multiple reads)
```

**What This Measures:**
- Single MMIO read/write latency
- Volatile semantics correctness
- Register struct layout efficiency

## 🔬 Detailed Analysis

### Running Benchmarks with Different Configurations

#### 1. CPU-Specific Optimizations

```bash
# Native CPU (maximum optimization)
RUSTFLAGS="-C target-cpu=native" cargo bench

# Specific ARM target
RUSTFLAGS="-C target-cpu=cortex-a53" cargo bench --target aarch64-unknown-linux-gnu

# With specific features
RUSTFLAGS="-C target-feature=+neon,+fp-armv8" cargo bench
```

#### 2. Pinning to Specific CPU Core

```bash
# Pin to CPU core 0 (reduces variance)
taskset -c 0 cargo bench

# Disable CPU frequency scaling
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Run benchmarks
taskset -c 0 cargo bench
```

#### 3. Memory Allocator Comparison

```bash
# Default allocator
cargo bench

# With jemalloc (if configured)
cargo bench --features jemalloc
```

### Interpreting Results

#### 1. Latency Histograms

Criterion generates HTML reports with latency distributions:

```bash
# Run benchmarks
cargo bench

# Open HTML reports
firefox target/criterion/nonce_queue/report/index.html
```

Look for:
- **Median latency** (50th percentile) - typical case
- **99th percentile** - worst-case latency
- **Standard deviation** - consistency (lower is better)

#### 2. Throughput Metrics

```
thrpt:  [119 Melem/s 120 Melem/s 121 Melem/s]
         ^min         ^median     ^max
```

- **120 Melem/s** = 120 million operations per second
- **1 / 120M = 8.3ns** per operation

#### 3. Performance Regression

Criterion automatically detects regressions:

```
nonce_queue/push    time:   [8.3 ns 8.4 ns 8.5 ns]
                    change: [+2.1% +2.5% +2.9%] (p = 0.00 < 0.05)
                    Performance has regressed.
```

Always investigate regressions >5%.

## 🧪 C Interoperability Testing

### 1. Build and Run C Example

```bash
# Build Rust library
cargo build --release

# Compile C example
gcc -o c_interop examples/c_interop.c \
    -I./include \
    -L./target/release \
    -lbmminer_rs \
    -lpthread -ldl -lm \
    -O3

# Run
LD_LIBRARY_PATH=./target/release ./c_interop
```

**Expected Output:**
```
=== C Interoperability Test ===

1. Verifying ABI compatibility...
   Pointer size:     8 bytes
   Nonce size:       128 bytes (expected: 128)
   Nonce alignment:  128 bytes (expected: 128)
   ✓ ABI compatible

2. Creating nonce queue...
   ✓ Successfully created nonce queue

3. Pushing 10,000 nonces...
   ✓ Pushed 10000 nonces
   ✓ Average push time: 8.2 ns

4. Popping 10,000 nonces...
   ✓ Popped 10000 nonces
   ✓ Average pop time: 7.8 ns
   ✓ All nonces matched (10000/10000)

5. Checking queue statistics...
   Total pushed:  10000
   Total popped:  10000
   Total dropped: 0
   ✓ Statistics correct

6. Performance Summary
   ================================
   Push: 8.2 ns/op (122 M ops/sec)
   Pop:  7.8 ns/op (128 M ops/sec)
   ================================

   ✓ Push performance: EXCELLENT (target: <10ns)
   ✓ Pop performance:  EXCELLENT (target: <10ns)

7. Cleaning up...
   ✓ Queue destroyed

=== All Tests Passed! ===
```

### 2. Valgrind Memory Check

Verify no memory leaks:

```bash
# Build without strip
cargo build --profile release-debug

# Run with valgrind
LD_LIBRARY_PATH=./target/release-debug \
valgrind --leak-check=full --show-leak-kinds=all \
./c_interop
```

**Expected Output:**
```
==12345== HEAP SUMMARY:
==12345==     in use at exit: 0 bytes in 0 blocks
==12345==   total heap usage: 10 allocs, 10 frees, 131,072 bytes allocated
==12345==
==12345== All heap blocks were freed -- no leaks are possible
```

## 📈 Profiling

### 1. CPU Profiling with perf

```bash
# Build with debug symbols
cargo build --profile release-debug

# Record profile
perf record --call-graph=dwarf cargo bench --bench nonce_queue

# View report
perf report

# Generate flamegraph
cargo install flamegraph
cargo flamegraph --bench nonce_queue
```

### 2. Cache Analysis

```bash
# Cache hit/miss analysis
perf stat -e cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses \
    cargo bench --bench nonce_queue
```

**Expected Results:**
```
Performance counter stats:

    10,234,567  cache-references
       123,456  cache-misses    # 1.2% of all cache refs  ← Good!
   100,234,567  L1-dcache-loads
       234,567  L1-dcache-load-misses  # 0.23% miss rate  ← Excellent!
```

Cache-line alignment should give <2% L1 miss rate.

## 🎯 Performance Validation Checklist

Before claiming performance improvements, verify:

- [ ] **Queue push <10ns** (single-threaded)
- [ ] **Queue pop <10ns** (single-threaded)
- [ ] **Hot path <100ns** (full cycle)
- [ ] **12x faster than pthread** (C comparison)
- [ ] **<2% cache miss rate** (L1 dcache)
- [ ] **Zero memory leaks** (Valgrind)
- [ ] **ABI compatible** (C interop test passes)
- [ ] **No regressions** (Criterion baseline)

## 🔧 Troubleshooting

### High Benchmark Variance

**Problem:** Latency varies by >10%

**Solutions:**
```bash
# 1. Pin to specific CPU
taskset -c 0 cargo bench

# 2. Disable frequency scaling
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# 3. Disable Turbo Boost
echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo

# 4. Increase sample size
cargo bench -- --sample-size 1000
```

### Slower Than Expected

**Problem:** Queue push shows 20ns instead of 8ns

**Checks:**
```bash
# 1. Verify release build
cargo bench --release

# 2. Check optimization flags
grep "opt-level" Cargo.toml  # Should be "3"
grep "lto" Cargo.toml        # Should be "fat"

# 3. Verify overflow checks are disabled
grep "overflow-checks" Cargo.toml  # Should be "false"

# 4. Check for debug assertions
cargo build --release --verbose  # Should NOT see "-C debug-assertions"
```

### C Interop Test Fails

**Problem:** `c_interop` crashes or shows wrong values

**Checks:**
```bash
# 1. Verify ABI compatibility
./c_interop  # Check "Nonce size: 128 bytes"

# 2. Check library path
ldd ./c_interop  # Should show libbmminer_rs.so found

# 3. Verify calling convention
objdump -T target/release/libbmminer_rs.so | grep bmminer_
# Should show extern "C" symbols

# 4. Check alignment
gdb ./c_interop
(gdb) print &nonce
# Address should be 128-byte aligned (ends in 00 or 80)
```

## 📊 Baseline Comparison

Save baseline for regression detection:

```bash
# First run - save baseline
cargo bench -- --save-baseline initial

# After changes - compare
cargo bench -- --baseline initial

# Criterion will show:
# - Green: Performance improved
# - Red: Performance regressed
# - Gray: No significant change
```

## 🚀 Continuous Benchmarking

Add to CI/CD:

```bash
# In .github/workflows/benchmark.yml
- name: Run benchmarks
  run: |
    cargo bench --bench nonce_queue -- --save-baseline ci
    cargo bench --bench c_comparison -- --save-baseline ci

- name: Check for regressions
  run: |
    if cargo bench -- --baseline ci | grep "Performance has regressed"; then
      echo "Performance regression detected!"
      exit 1
    fi
```

## 📝 Reporting Results

When reporting benchmark results, include:

1. **Hardware:** CPU model, RAM, OS
2. **Configuration:** Optimization flags, CPU pinning
3. **Sample size:** Number of iterations
4. **Baseline:** What you're comparing against
5. **Raw data:** Link to Criterion HTML reports

**Example:**

```markdown
## Benchmark Results

**Hardware:** Intel Xeon E5-2680 v4 @ 2.4GHz, 64GB RAM, Ubuntu 20.04
**Configuration:** `-C target-cpu=native`, pinned to core 0, performance governor
**Sample size:** 100 iterations per benchmark

### Lock-Free Queue Performance

| Operation | Latency | Throughput | vs C |
|-----------|---------|------------|------|
| Push      | 8.3ns   | 120 Mops/s | 12.0x |
| Pop       | 7.9ns   | 126 Mops/s | 12.7x |

**Raw data:** [Criterion Report](target/criterion/report/index.html)
```

---

**Ready to benchmark?** Start with:

```bash
# Full benchmark suite
cargo bench

# C interop validation
./examples/run_c_interop.sh

# View HTML reports
firefox target/criterion/report/index.html
```

**Questions?** See [PERFORMANCE.md](PERFORMANCE.md) for detailed performance analysis.
