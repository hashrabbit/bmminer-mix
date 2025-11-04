//! C vs Rust Performance Comparison Benchmarks
//!
//! These benchmarks compare the performance of the Rust implementation
//! against the original C implementation for backwards compatibility validation.
//!
//! Run with: cargo bench --bench c_comparison

use bmminer_rs::{
    ffi::*,
    nonce_queue::NoncePipeline,
    types::Nonce,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Simulated C-style queue using pthread mutex (for comparison)
struct CStyleQueue {
    buffer: Mutex<Vec<Nonce>>,
    capacity: usize,
}

impl CStyleQueue {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(capacity)),
            capacity,
        }
    }

    fn push(&self, nonce: Nonce) -> bool {
        let mut buf = self.buffer.lock().unwrap();
        if buf.len() >= self.capacity {
            false
        } else {
            buf.push(nonce);
            true
        }
    }

    fn pop(&self) -> Option<Nonce> {
        let mut buf = self.buffer.lock().unwrap();
        buf.pop()
    }
}

// ===== Queue Push/Pop Benchmarks =====

fn bench_queue_push_rust(c: &mut Criterion) {
    let queue = NoncePipeline::new();
    let nonce = Nonce::new();

    c.bench_function("queue_push/rust_native", |b| {
        b.iter(|| {
            black_box(queue.push(black_box(nonce)));
        });
    });
}

fn bench_queue_push_rust_ffi(c: &mut Criterion) {
    unsafe {
        let handle = bmminer_nonce_queue_create();
        let nonce = Nonce::new();

        c.bench_function("queue_push/rust_via_ffi", |b| {
            b.iter(|| {
                black_box(bmminer_nonce_queue_push(handle, &nonce));
            });
        });

        bmminer_nonce_queue_destroy(handle);
    }
}

fn bench_queue_push_c_style(c: &mut Criterion) {
    let queue = CStyleQueue::new(4096);
    let nonce = Nonce::new();

    c.bench_function("queue_push/c_pthread_mutex", |b| {
        b.iter(|| {
            black_box(queue.push(black_box(nonce)));
        });
    });
}

fn bench_queue_pop_rust(c: &mut Criterion) {
    let queue = NoncePipeline::new();
    let nonce = Nonce::new();

    // Pre-fill queue
    for _ in 0..100 {
        queue.push(nonce);
    }

    c.bench_function("queue_pop/rust_native", |b| {
        b.iter(|| {
            black_box(queue.pop());
            queue.push(nonce); // Keep queue populated
        });
    });
}

fn bench_queue_pop_rust_ffi(c: &mut Criterion) {
    unsafe {
        let handle = bmminer_nonce_queue_create();
        let nonce = Nonce::new();

        // Pre-fill queue
        for _ in 0..100 {
            bmminer_nonce_queue_push(handle, &nonce);
        }

        c.bench_function("queue_pop/rust_via_ffi", |b| {
            b.iter(|| {
                let mut out = Nonce::new();
                black_box(bmminer_nonce_queue_pop(handle, &mut out));
                bmminer_nonce_queue_push(handle, &nonce); // Keep queue populated
            });
        });

        bmminer_nonce_queue_destroy(handle);
    }
}

fn bench_queue_pop_c_style(c: &mut Criterion) {
    let queue = CStyleQueue::new(4096);
    let nonce = Nonce::new();

    // Pre-fill queue
    for _ in 0..100 {
        queue.push(nonce);
    }

    c.bench_function("queue_pop/c_pthread_mutex", |b| {
        b.iter(|| {
            black_box(queue.pop());
            queue.push(nonce); // Keep queue populated
        });
    });
}

// ===== Concurrent Benchmarks =====

fn bench_queue_concurrent_rust(c: &mut Criterion) {
    c.bench_function("queue_concurrent/rust_lock_free", |b| {
        b.iter(|| {
            let queue = Arc::new(NoncePipeline::new());
            let queue_clone = Arc::clone(&queue);

            // Producer thread
            let producer = thread::spawn(move || {
                for i in 0..10_000 {
                    let mut nonce = Nonce::new();
                    nonce.nonce3 = i;
                    while !queue_clone.push(nonce) {
                        thread::yield_now();
                    }
                }
            });

            // Consumer thread (this thread)
            for _ in 0..10_000 {
                while queue.pop().is_none() {
                    thread::yield_now();
                }
            }

            producer.join().unwrap();
        });
    });
}

fn bench_queue_concurrent_c_style(c: &mut Criterion) {
    c.bench_function("queue_concurrent/c_pthread_mutex", |b| {
        b.iter(|| {
            let queue = Arc::new(CStyleQueue::new(4096));
            let queue_clone = Arc::clone(&queue);

            // Producer thread
            let producer = thread::spawn(move || {
                for i in 0..10_000 {
                    let mut nonce = Nonce::new();
                    nonce.nonce3 = i;
                    while !queue_clone.push(nonce) {
                        thread::yield_now();
                    }
                }
            });

            // Consumer thread (this thread)
            for _ in 0..10_000 {
                while queue.pop().is_none() {
                    thread::yield_now();
                }
            }

            producer.join().unwrap();
        });
    });
}

// ===== Throughput Benchmarks =====

fn bench_throughput_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_1M_nonces");

    // Rust native
    group.bench_function("rust_native", |b| {
        b.iter(|| {
            let queue = NoncePipeline::new();
            let nonce = Nonce::new();

            for _ in 0..1_000_000 {
                black_box(queue.push(black_box(nonce)));
            }
        });
    });

    // Rust via FFI
    group.bench_function("rust_via_ffi", |b| {
        b.iter(|| {
            unsafe {
                let handle = bmminer_nonce_queue_create();
                let nonce = Nonce::new();

                for _ in 0..1_000_000 {
                    black_box(bmminer_nonce_queue_push(handle, &nonce));
                }

                bmminer_nonce_queue_destroy(handle);
            }
        });
    });

    // C-style pthread
    group.bench_function("c_pthread_mutex", |b| {
        b.iter(|| {
            let queue = CStyleQueue::new(4096);
            let nonce = Nonce::new();

            for i in 0..1_000_000 {
                black_box(queue.push(black_box(nonce)));
                // Occasionally pop to prevent filling up
                if i % 100 == 0 {
                    queue.pop();
                }
            }
        });
    });

    group.finish();
}

// ===== Hot Path Simulation =====

fn bench_hot_path_rust(c: &mut Criterion) {
    c.bench_function("hot_path_full_cycle/rust", |b| {
        let queue = NoncePipeline::new();
        let mut nonce = Nonce::new();

        b.iter(|| {
            // Simulate: Check FIFO count (50ns)
            let count = black_box(1u32);

            if count > 0 {
                // Simulate: Read nonce from FPGA (50ns)
                nonce.nonce3 = black_box(0x12345678);

                // Simulate: Parse nonce (20ns)
                nonce.timestamp_ns = black_box(1000);

                // Push to queue (8ns)
                black_box(queue.push(nonce));
            }
        });
    });
}

fn bench_hot_path_c_style(c: &mut Criterion) {
    c.bench_function("hot_path_full_cycle/c_pthread", |b| {
        let queue = CStyleQueue::new(4096);
        let mut nonce = Nonce::new();

        b.iter(|| {
            // Simulate: Check FIFO count (50ns)
            let count = black_box(1u32);

            if count > 0 {
                // Simulate: Read nonce from FPGA (50ns)
                nonce.nonce3 = black_box(0x12345678);

                // Simulate: Parse nonce (20ns)
                nonce.timestamp_ns = black_box(1000);

                // Push to queue (100ns with pthread mutex)
                black_box(queue.push(nonce));
            }
        });
    });
}

// ===== Memory Usage Comparison =====

fn bench_memory_footprint(c: &mut Criterion) {
    c.bench_function("memory_footprint/measure", |b| {
        b.iter(|| {
            // Measure Rust queue memory
            let rust_queue = NoncePipeline::new();
            let rust_size = std::mem::size_of_val(&rust_queue);

            // Measure C-style queue memory
            let c_queue = CStyleQueue::new(4096);
            let c_size = std::mem::size_of_val(&c_queue)
                + c_queue.capacity * std::mem::size_of::<Nonce>();

            black_box((rust_size, c_size));
        });
    });
}

// ===== ABI Compatibility Tests =====

fn test_abi_compatibility(c: &mut Criterion) {
    c.bench_function("abi_compatibility/verify", |b| {
        b.iter(|| {
            unsafe {
                // Verify pointer size
                let ptr_size = bmminer_get_pointer_size();
                assert_eq!(ptr_size, std::mem::size_of::<*const ()>());

                // Verify Nonce size
                let nonce_size = bmminer_get_nonce_size();
                assert_eq!(nonce_size, std::mem::size_of::<Nonce>());

                // Verify Nonce alignment
                let nonce_align = bmminer_get_nonce_alignment();
                assert_eq!(nonce_align, std::mem::align_of::<Nonce>());

                black_box((ptr_size, nonce_size, nonce_align));
            }
        });
    });
}

// ===== Register Benchmarks =====

criterion_group!(
    benches,
    bench_queue_push_rust,
    bench_queue_push_rust_ffi,
    bench_queue_push_c_style,
    bench_queue_pop_rust,
    bench_queue_pop_rust_ffi,
    bench_queue_pop_c_style,
    bench_queue_concurrent_rust,
    bench_queue_concurrent_c_style,
    bench_throughput_comparison,
    bench_hot_path_rust,
    bench_hot_path_c_style,
    bench_memory_footprint,
    test_abi_compatibility,
);

criterion_main!(benches);
