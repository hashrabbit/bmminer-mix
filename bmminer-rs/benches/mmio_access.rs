//! Benchmark for MMIO register access
//!
//! This measures the raw cost of reading FPGA registers.
//!
//! Note: These benchmarks use simulated MMIO (heap memory) since
//! we can't access real hardware in benchmarks. Real FPGA latency
//! will be higher (~50ns vs these ~1-2ns results).

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::sync::atomic::{AtomicU32, Ordering};

/// Simulated FPGA register (using atomic for volatile semantics)
struct SimulatedFpgaReg {
    value: AtomicU32,
}

impl SimulatedFpgaReg {
    fn new(init: u32) -> Self {
        Self {
            value: AtomicU32::new(init),
        }
    }

    #[inline(always)]
    fn read(&self) -> u32 {
        self.value.load(Ordering::SeqCst) // Simulates volatile read
    }

    #[inline(always)]
    fn write(&self, val: u32) {
        self.value.store(val, Ordering::SeqCst) // Simulates volatile write
    }
}

fn bench_single_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmio/single_read");
    group.throughput(Throughput::Elements(1));

    let reg = SimulatedFpgaReg::new(0x12345678);

    group.bench_function("atomic_load", |b| {
        b.iter(|| {
            let val = reg.read();
            black_box(val);
        });
    });

    group.finish();
}

fn bench_single_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmio/single_write");
    group.throughput(Throughput::Elements(1));

    let reg = SimulatedFpgaReg::new(0);

    group.bench_function("atomic_store", |b| {
        b.iter(|| {
            reg.write(black_box(0xDEADBEEF));
        });
    });

    group.finish();
}

fn bench_nonce_read_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmio/nonce_read_full");
    group.throughput(Throughput::Elements(1));

    // Simulate reading full nonce data (multiple registers)
    let regs = vec![
        SimulatedFpgaReg::new(0x12345678), // nonce3
        SimulatedFpgaReg::new(0x00000042), // job_id
        SimulatedFpgaReg::new(0x00000001), // header_version
        SimulatedFpgaReg::new(0xAABBCCDD), // nonce2_lo
        SimulatedFpgaReg::new(0x11223344), // nonce2_hi
    ];

    group.bench_function("five_registers", |b| {
        b.iter(|| {
            let nonce3 = regs[0].read();
            let job_id = regs[1].read();
            let header_version = regs[2].read();
            let nonce2_lo = regs[3].read() as u64;
            let nonce2_hi = regs[4].read() as u64;
            let nonce2 = (nonce2_hi << 32) | nonce2_lo;

            black_box((nonce3, job_id, header_version, nonce2));
        });
    });

    group.finish();
}

fn bench_hot_path_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmio/hot_path");
    group.throughput(Throughput::Elements(1));

    let fifo_count = SimulatedFpgaReg::new(1);
    let return_nonce = SimulatedFpgaReg::new(0x12345678);

    group.bench_function("check_and_read", |b| {
        b.iter(|| {
            // Simulate hot path: check FIFO count, read nonce
            let count = fifo_count.read();
            if count > 0 {
                let nonce = return_nonce.read();
                black_box(nonce);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_read,
    bench_single_write,
    bench_nonce_read_simulation,
    bench_hot_path_simulation,
);
criterion_main!(benches);
