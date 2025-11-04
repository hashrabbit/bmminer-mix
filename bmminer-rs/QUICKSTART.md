# 🚀 Quick Start Guide

Get up and running with BMminer-RS in 5 minutes!

## Step 1: Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version  # Verify installation
```

## Step 2: Clone or Copy Project

```bash
# If you have it locally
cd /home/user/bmminer-rs

# Verify structure
ls -la
# Should see: Cargo.toml, src/, benches/, build.sh
```

## Step 3: Build (Choose One)

### Option A: Development Build (Fast compile, slower runtime)
```bash
cargo build
# Binary: target/debug/libbmminer_rs.rlib
```

### Option B: Release Build (Slow compile, MAXIMUM SPEED)
```bash
cargo build --release
# Binary: target/release/libbmminer_rs.rlib
```

### Option C: Use Build Script (Recommended)
```bash
./build.sh release
```

## Step 4: Run Tests

```bash
cargo test

# You should see:
#   test types::tests::nonce_size_alignment ... ok
#   test nonce_queue::tests::test_push_pop ... ok
#   test nonce_queue::tests::test_queue_full ... ok
#   test nonce_queue::tests::test_concurrent_push_pop ... ok
# All tests passed!
```

## Step 5: Run Benchmarks

```bash
cargo bench

# This will take 5-10 minutes
# Results will be in: target/criterion/
```

### Expected Results:
```
nonce_queue/push/single_threaded
    time: [8.234 ns 8.301 ns 8.372 ns]
    thrpt: [119.43 Melem/s 120.47 Melem/s 121.46 Melem/s]

nonce_queue/pop/single_threaded
    time: [7.982 ns 8.045 ns 8.115 ns]
    thrpt: [123.22 Melem/s 124.30 Melem/s 125.29 Melem/s]
```

**Translation:** ~8ns per operation = **125 million nonces/sec!** 🚀

## Step 6: View Benchmark Reports (Optional)

```bash
# Open HTML reports in browser
firefox target/criterion/nonce_queue/push/single_threaded/report/index.html

# Or on macOS
open target/criterion/nonce_queue/push/single_threaded/report/index.html
```

## Step 7: Cross-Compile for ARM (Bitmain Hardware)

### Install ARM Toolchain

```bash
# Install Rust ARM target
rustup target add aarch64-unknown-linux-gnu

# Install GCC cross-compiler (Ubuntu/Debian)
sudo apt-get install gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu

# Verify
aarch64-linux-gnu-gcc --version
```

### Build for ARM

```bash
./build.sh arm

# Binary will be at:
# target/aarch64-unknown-linux-gnu/release/libbmminer_rs.rlib
```

### Deploy to Bitmain Device

```bash
# Copy binary to device
scp target/aarch64-unknown-linux-gnu/release/libbmminer_rs.rlib root@192.168.1.100:/tmp/

# SSH into device
ssh root@192.168.1.100

# Test on device
# (You'll need to integrate with existing C code or write a main.rs)
```

## Common Issues

### Issue: "command not found: cargo"

**Solution:** Install Rust or add to PATH:
```bash
source $HOME/.cargo/env
```

### Issue: "failed to compile bmminer-rs"

**Solution:** Check Rust version (need 1.70+):
```bash
rustc --version
rustup update
```

### Issue: "Permission denied on /dev/mem"

**Solution:** Hardware access requires root or capabilities:
```bash
# Grant CAP_SYS_RAWIO capability
sudo setcap cap_sys_rawio+ep ./target/release/your_binary

# Or run as root (not recommended)
sudo ./target/release/your_binary
```

### Issue: Benchmarks show high variance

**Solution:** Pin to specific CPU core and disable frequency scaling:
```bash
# Pin to core 0
taskset -c 0 cargo bench

# Disable CPU frequency scaling (requires root)
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

### Issue: Cross-compilation fails

**Solution:** Ensure toolchain is installed:
```bash
# Reinstall ARM toolchain
sudo apt-get install --reinstall gcc-aarch64-linux-gnu

# Verify linker
which aarch64-linux-gnu-gcc

# Set environment variables explicitly
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar
```

## Next Steps

### 1. **Integrate with Existing C Code**

Create a FFI (Foreign Function Interface) wrapper:

```rust
// src/ffi.rs
#[no_mangle]
pub extern "C" fn bmminer_queue_create() -> *mut NoncePipeline {
    Box::into_raw(Box::new(NoncePipeline::new()))
}

#[no_mangle]
pub extern "C" fn bmminer_queue_push(
    queue: *mut NoncePipeline,
    nonce: *const Nonce,
) -> bool {
    unsafe {
        (*queue).push(*nonce)
    }
}
```

Then call from C:
```c
extern void* bmminer_queue_create();
extern bool bmminer_queue_push(void* queue, const struct nonce* n);

void* queue = bmminer_queue_create();
struct nonce n = { ... };
bmminer_queue_push(queue, &n);
```

### 2. **Profile on Real Hardware**

```bash
# Build with debug symbols
cargo build --release

# Profile with perf
perf record --call-graph=dwarf ./your_binary
perf report

# Generate flamegraph
cargo install flamegraph
cargo flamegraph
```

### 3. **Optimize Further**

Based on profiling results:
- Enable CPU-specific features (`-C target-cpu=native`)
- Use huge pages for large allocations
- Add SIMD instructions for SHA-256
- Tune cache alignment

### 4. **Test Under Load**

```bash
# Stress test
stress-ng --cpu 4 --io 4 --vm 2 --vm-bytes 1G --timeout 60s &
cargo bench  # See if performance degrades
```

## Performance Checklist

Before deploying to production, verify:

- [ ] Benchmarks show <10ns queue operations
- [ ] Tests pass (cargo test)
- [ ] No memory leaks (valgrind --leak-check=full)
- [ ] CPU usage acceptable (<300% for full load)
- [ ] Temperature stays under limits
- [ ] Hashrate matches or exceeds C version
- [ ] No crashes after 24hr stress test
- [ ] Power consumption acceptable

## Resources

- **Rust Book:** https://doc.rust-lang.org/book/
- **Criterion.rs:** https://bheisler.github.io/criterion.rs/book/
- **Lock-Free Programming:** https://preshing.com/20120612/an-introduction-to-lock-free-programming/
- **Performance Guide:** https://nnethercote.github.io/perf-book/

## Support

Questions? Check:
1. **README.md** - Architecture and overview
2. **PERFORMANCE.md** - Detailed benchmarks
3. **IMPLEMENTATION_SUMMARY.md** - What was built

## Success Criteria

You're ready to integrate when:

✅ All tests pass
✅ Benchmarks show expected performance
✅ Cross-compilation works
✅ You understand the hot path (nonce_queue.rs)
✅ You can explain why it's faster than C

**Now go make some money!** 💰🚀

---

*Estimated time to working prototype: 1 week*
*Estimated time to production: 3-4 months*
