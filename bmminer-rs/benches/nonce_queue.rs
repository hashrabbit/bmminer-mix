//! Benchmark for lock-free nonce queue
//!
//! This proves the queue is fast enough for the hot path.
//!
//! Target: <10ns per push/pop operation

use bmminer_rs::{Nonce, NoncePipeline};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

fn bench_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_queue/push");
    group.throughput(Throughput::Elements(1));

    let queue = NoncePipeline::new();
    let nonce = Nonce::default();

    group.bench_function("single_threaded", |b| {
        b.iter(|| {
            // Push and pop to avoid filling queue
            queue.push(black_box(nonce));
            queue.pop();
        });
    });

    group.finish();
}

fn bench_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_queue/pop");
    group.throughput(Throughput::Elements(1));

    let queue = NoncePipeline::new();
    let nonce = Nonce::default();

    // Pre-fill queue
    for _ in 0..1000 {
        queue.push(nonce);
    }

    group.bench_function("single_threaded", |b| {
        b.iter(|| {
            if let Some(n) = queue.pop() {
                black_box(n);
            }
            // Push to avoid emptying queue
            queue.push(nonce);
        });
    });

    group.finish();
}

fn bench_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_queue/concurrent");

    for &batch_size in &[1, 10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size));

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let queue = Arc::new(NoncePipeline::new());
                    let queue_clone = queue.clone();

                    let producer = thread::spawn(move || {
                        for i in 0..batch_size {
                            let mut nonce = Nonce::default();
                            nonce.nonce3 = i as u32;

                            while !queue.push(nonce) {
                                core::hint::spin_loop();
                            }
                        }
                    });

                    let consumer = thread::spawn(move || {
                        let mut count = 0;
                        while count < batch_size {
                            if let Some(_) = queue_clone.pop() {
                                count += 1;
                            } else {
                                core::hint::spin_loop();
                            }
                        }
                    });

                    producer.join().unwrap();
                    consumer.join().unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_queue/throughput");
    group.sample_size(10); // Fewer samples for long-running tests

    const NONCES: u64 = 1_000_000;
    group.throughput(Throughput::Elements(NONCES));

    group.bench_function("million_nonces", |b| {
        b.iter(|| {
            let queue = Arc::new(NoncePipeline::new());
            let queue_clone = queue.clone();

            let start = std::time::Instant::now();

            let producer = thread::spawn(move || {
                for i in 0..NONCES {
                    let mut nonce = Nonce::default();
                    nonce.nonce3 = i as u32;

                    while !queue.push(nonce) {
                        core::hint::spin_loop();
                    }
                }
            });

            let consumer = thread::spawn(move || {
                let mut count = 0;
                while count < NONCES {
                    if let Some(_) = queue_clone.pop() {
                        count += 1;
                    } else {
                        core::hint::spin_loop();
                    }
                }
            });

            producer.join().unwrap();
            consumer.join().unwrap();

            let duration = start.elapsed();
            let nonces_per_sec = NONCES as f64 / duration.as_secs_f64();

            println!(
                "\nProcessed {} million nonces in {:?} ({:.0} nonces/sec)",
                NONCES / 1_000_000,
                duration,
                nonces_per_sec
            );
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_push,
    bench_pop,
    bench_concurrent,
    bench_throughput
);
criterion_main!(benches);
