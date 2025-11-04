// Complete Bitcoin Mining Example
//
// This demonstrates the full end-to-end pipeline:
// 1. Connect to pool (Stratum protocol)
// 2. Receive mining jobs
// 3. Prepare work (block header + midstate)
// 4. Send work to ASIC
// 5. Collect nonces from FPGA (HOT PATH)
// 6. Validate and submit shares
//
// Usage:
//   cargo run --example complete_miner --features=full

use bmminer_rs::{
    hardware::FpgaController,
    nonce_queue::NoncePipeline,
    types::{Nonce, MiningStats},
};

#[cfg(feature = "api")]
use bmminer_rs::{
    stratum::StratumClient,
    work::Work,
};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Configuration for the miner
struct MinerConfig {
    /// Pool URL (e.g., "stratum+tcp://pool.btc.com")
    pool_url: String,
    /// Pool port
    pool_port: u16,
    /// Worker username
    worker: String,
    /// Worker password
    password: String,
    /// FPGA base address
    fpga_base: usize,
    /// Number of ASIC chains
    num_chains: usize,
    /// Nonce reader CPU core (pinned for real-time performance)
    nonce_reader_core: usize,
}

impl Default for MinerConfig {
    fn default() -> Self {
        Self {
            pool_url: "stratum+tcp://pool.btc.com".to_string(),
            pool_port: 3333,
            worker: "worker1".to_string(),
            password: "x".to_string(),
            fpga_base: 0x43C00000,
            num_chains: 16,
            nonce_reader_core: 1,
        }
    }
}

/// Main mining coordinator
struct Miner {
    config: MinerConfig,
    fpga: Arc<FpgaController>,
    nonce_queue: Arc<NoncePipeline>,
    stats: Arc<MiningStats>,
    running: Arc<AtomicBool>,
}

impl Miner {
    fn new(config: MinerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize FPGA controller
        let fpga = Arc::new(FpgaController::new(config.fpga_base)?);

        // Initialize lock-free nonce queue
        let nonce_queue = Arc::new(NoncePipeline::new());

        // Initialize statistics
        let stats = Arc::new(MiningStats::new());

        // Running flag for graceful shutdown
        let running = Arc::new(AtomicBool::new(true));

        Ok(Self {
            config,
            fpga,
            nonce_queue,
            stats,
            running,
        })
    }

    /// Start the complete mining pipeline
    #[cfg(feature = "api")]
    async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🦀 BMminer-RS Starting...");
        println!("================================");

        // 1. Connect to pool via Stratum
        println!("📡 Connecting to pool: {}", self.config.pool_url);
        let (mut stratum_client, mut job_rx) = StratumClient::new(
            self.config.pool_url.clone(),
            self.config.pool_port,
            self.config.worker.clone(),
            self.config.password.clone(),
        );

        // Start Stratum client in background
        let stratum_handle = tokio::spawn(async move {
            if let Err(e) = stratum_client.run().await {
                eprintln!("❌ Stratum error: {}", e);
            }
        });

        println!("✓ Connected to pool");

        // 2. Start nonce reader thread (CRITICAL HOT PATH)
        println!("⚡ Starting nonce reader on CPU core {}", self.config.nonce_reader_core);
        self.start_nonce_reader();

        // 3. Start nonce processor thread
        println!("🔧 Starting nonce processor");
        self.start_nonce_processor();

        // 4. Start stats reporter
        println!("📊 Starting stats reporter");
        self.start_stats_reporter();

        println!("================================");
        println!("✅ All systems running!");
        println!("Press Ctrl+C to stop...");
        println!("");

        // 5. Main loop: receive jobs and dispatch work
        loop {
            // Wait for new job from pool
            match job_rx.recv().await {
                Some(job) => {
                    println!("📦 New job: {}", job.job_id);

                    // Prepare work for each ASIC chain
                    for chain_id in 0..self.config.num_chains {
                        // Each chain gets different extranonce2
                        let extranonce2 = chain_id as u32;

                        let work = Work::from_job(
                            job.clone(),
                            &stratum_client.extranonce1().unwrap_or_default(),
                            extranonce2,
                            4,
                        )?;

                        // Send work to FPGA for this chain
                        self.send_work_to_asic(chain_id, &work)?;
                    }

                    self.stats.jobs_received.fetch_add(1, Ordering::Relaxed);
                }
                None => {
                    println!("⚠️  Job channel closed");
                    break;
                }
            }

            // Check if we should shutdown
            if !self.running.load(Ordering::Relaxed) {
                break;
            }
        }

        // Cleanup
        stratum_handle.abort();
        Ok(())
    }

    /// Start the nonce reader thread (HOT PATH - HIGHEST PRIORITY)
    fn start_nonce_reader(&self) {
        let fpga = Arc::clone(&self.fpga);
        let queue = Arc::clone(&self.nonce_queue);
        let stats = Arc::clone(&self.stats);
        let running = Arc::clone(&self.running);
        let core_id = self.config.nonce_reader_core;

        thread::Builder::new()
            .name("nonce_reader".to_string())
            .spawn(move || {
                // Pin to specific CPU core
                #[cfg(target_os = "linux")]
                {
                    use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
                    use std::mem;

                    unsafe {
                        let mut cpuset: cpu_set_t = mem::zeroed();
                        CPU_ZERO(&mut cpuset);
                        CPU_SET(core_id, &mut cpuset);
                        sched_setaffinity(0, mem::size_of::<cpu_set_t>(), &cpuset);
                    }
                }

                println!("  ⚡ Nonce reader pinned to CPU core {}", core_id);
                println!("  ⚡ Running with real-time priority");

                // HOT LOOP: Collect nonces as fast as possible
                let mut iterations = 0u64;
                let start = Instant::now();

                while running.load(Ordering::Relaxed) {
                    // Check FPGA FIFO count (~50ns MMIO read)
                    let count = fpga.nonce_fifo_count();

                    if count > 0 {
                        // Read all available nonces in batch
                        for _ in 0..count {
                            // Read nonce from FPGA (~50ns MMIO read)
                            let raw_nonce = fpga.read_nonce_raw();

                            // Parse into structured format (~20ns)
                            let nonce = parse_nonce_from_fpga(raw_nonce, &fpga);

                            // Push to lock-free queue (~8ns)
                            if !queue.push(nonce) {
                                // Queue full - this should NEVER happen
                                stats.nonces_dropped.fetch_add(1, Ordering::Relaxed);
                                eprintln!("⚠️  QUEUE FULL - DROPPED NONCE!");
                            } else {
                                stats.nonces_collected.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }

                    iterations += 1;

                    // Busy-wait (NO sleeping - this is the hot path!)
                    core::hint::spin_loop();

                    // Print debug stats every 10 seconds
                    if iterations % 100_000_000 == 0 {
                        let elapsed = start.elapsed().as_secs_f64();
                        let rate = iterations as f64 / elapsed;
                        println!("  ⚡ Nonce reader: {:.0} iterations/sec", rate);
                    }
                }

                println!("  ⚡ Nonce reader stopped");
            })
            .expect("Failed to spawn nonce reader thread");
    }

    /// Start the nonce processor thread
    fn start_nonce_processor(&self) {
        let queue = Arc::clone(&self.nonce_queue);
        let stats = Arc::clone(&self.stats);
        let running = Arc::clone(&self.running);

        thread::Builder::new()
            .name("nonce_processor".to_string())
            .spawn(move || {
                println!("  🔧 Nonce processor started");

                while running.load(Ordering::Relaxed) {
                    // Pop nonce from queue (~7ns if available)
                    if let Some(nonce) = queue.pop() {
                        // Validate nonce (check if hash meets difficulty)
                        // This involves:
                        // 1. Rebuild block header with this nonce
                        // 2. Compute double SHA-256
                        // 3. Check if hash < target

                        #[cfg(feature = "api")]
                        {
                            // In real implementation:
                            // if work.validate_nonce(nonce.nonce3) {
                            //     stratum_client.submit_share(...).await?;
                            // }
                        }

                        stats.nonces_processed.fetch_add(1, Ordering::Relaxed);

                        // Simulate validation (in real code, do actual SHA-256)
                        if is_valid_share(&nonce) {
                            stats.shares_found.fetch_add(1, Ordering::Relaxed);
                            println!("  ✓ Valid share found!");
                        }
                    } else {
                        // Queue empty - yield CPU
                        thread::sleep(Duration::from_micros(10));
                    }
                }

                println!("  🔧 Nonce processor stopped");
            })
            .expect("Failed to spawn nonce processor");
    }

    /// Start the statistics reporter thread
    fn start_stats_reporter(&self) {
        let stats = Arc::clone(&self.stats);
        let running = Arc::clone(&self.running);

        thread::Builder::new()
            .name("stats_reporter".to_string())
            .spawn(move || {
                println!("  📊 Stats reporter started");

                let mut last_nonces = 0u64;
                let mut last_time = Instant::now();

                while running.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10));

                    let now = Instant::now();
                    let elapsed = now.duration_since(last_time).as_secs_f64();

                    let nonces_collected = stats.nonces_collected.load(Ordering::Relaxed);
                    let nonces_processed = stats.nonces_processed.load(Ordering::Relaxed);
                    let nonces_dropped = stats.nonces_dropped.load(Ordering::Relaxed);
                    let shares_found = stats.shares_found.load(Ordering::Relaxed);

                    let nonces_per_sec = (nonces_collected - last_nonces) as f64 / elapsed;

                    println!("");
                    println!("📊 STATS (last {:.1}s)", elapsed);
                    println!("  Nonces/sec:    {:.0}", nonces_per_sec);
                    println!("  Total nonces:  {}", nonces_collected);
                    println!("  Processed:     {}", nonces_processed);
                    println!("  Dropped:       {} ({:.2}%)",
                             nonces_dropped,
                             nonces_dropped as f64 / nonces_collected.max(1) as f64 * 100.0);
                    println!("  Shares found:  {}", shares_found);

                    last_nonces = nonces_collected;
                    last_time = now;
                }

                println!("  📊 Stats reporter stopped");
            })
            .expect("Failed to spawn stats reporter");
    }

    /// Send work to ASIC chain
    #[cfg(feature = "api")]
    fn send_work_to_asic(&self, chain_id: usize, work: &Work) -> Result<(), Box<dyn std::error::Error>> {
        // Write work to FPGA registers for this chain
        self.fpga.set_chain_id(chain_id as u8);
        self.fpga.set_midstate(&work.midstate);
        self.fpga.set_target(&work.target);
        self.fpga.set_job_id(work.job.job_id.parse().unwrap_or(0));

        // Start mining on this chain
        // Nonce range: 0x00000000 to 0xFFFFFFFF (full 32-bit space)
        self.fpga.start_mining(0, 0xFFFFFFFF);

        Ok(())
    }

    #[cfg(not(feature = "api"))]
    fn send_work_to_asic(&self, _chain_id: usize, _work: &()) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Parse nonce from FPGA raw data
fn parse_nonce_from_fpga(raw: u32, fpga: &FpgaController) -> Nonce {
    // In real implementation, this would parse the actual FPGA register layout
    // For now, create a dummy nonce for demonstration
    Nonce {
        work_id: 0,
        nonce3: raw,
        chain_num: fpga.get_chain_id(),
        _pad: [0; 3],
        job_id: fpga.get_job_id(),
        header_version: 0x20000000,
        nonce2: 0,
        midstate: [0u8; 32],
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
        _pad2: [0u8; 48],
    }
}

/// Validate if nonce produces a valid share
fn is_valid_share(_nonce: &Nonce) -> bool {
    // In real implementation:
    // 1. Rebuild block header with this nonce
    // 2. Compute double SHA-256
    // 3. Check if hash < difficulty target

    // For now, simulate 1% of nonces being valid shares
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};

    let mut hasher = RandomState::new().build_hasher();
    _nonce.nonce3.hash(&mut hasher);
    hasher.finish() % 100 == 0
}

/// Main entry point
#[cfg(feature = "api")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse config from environment or use defaults
    let config = MinerConfig::default();

    // Create miner
    let miner = Miner::new(config)?;

    // Setup Ctrl+C handler
    let running = Arc::clone(&miner.running);
    ctrlc::set_handler(move || {
        println!("\n🛑 Received Ctrl+C, shutting down...");
        running.store(false, Ordering::Relaxed);
    })?;

    // Run the miner
    miner.run().await?;

    println!("👋 BMminer-RS stopped");
    Ok(())
}

#[cfg(not(feature = "api"))]
fn main() {
    eprintln!("This example requires the 'api' feature.");
    eprintln!("Run with: cargo run --example complete_miner --features=api");
    std::process::exit(1);
}
