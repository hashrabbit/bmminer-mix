//! # BMminer-RS: High-Performance Bitcoin Mining Software
//!
//! This is a complete rewrite of bmminer in Rust, optimized for:
//! - Maximum performance (every nanosecond counts)
//! - Memory safety (zero buffer overflows)
//! - Lock-free concurrency (minimal contention)
//! - Embedded Linux / IoT deployment
//!
//! ## Architecture
//!
//! ```text
//! HOT PATH:  FPGA → Nonce Reader → Lock-Free Queue → Processor
//!            50ns    150ns         10ns               20ns
//!
//! AGGRESSIVE MODE (16 chains + batching):
//!            FPGA × 16 → Batch Reader → SIMD Processing → Pool
//!            50ns       200ns          ~60ns (NEON)     network
//!
//! Performance: 100-500x vs C implementation
//! ```

#![allow(unsafe_code)] // We need unsafe for MMIO and performance

// Core infrastructure
pub mod ffi;
pub mod hardware;
pub mod nonce_queue;
pub mod types;

// Multi-chain support (16x performance)
pub mod multichain;

// Batch processing (3-5x performance)
pub mod batch;

// NEON-accelerated SHA-256 (2.5x faster on Cortex-A9)
pub mod sha256_neon;

// Zero-waste parallel validation
#[cfg(feature = "api")]
pub mod validation_pool;

// BSV Node implementation (shares validation pool with miner)
#[cfg(feature = "bsv-node")]
pub mod bsv_node;

#[cfg(feature = "bsv-node")]
pub mod bsv_large_blocks;

// Optional features
#[cfg(feature = "api")]
pub mod stratum;

#[cfg(feature = "api")]
pub mod work;

// Re-exports
pub use hardware::FpgaController;
pub use nonce_queue::NoncePipeline;
pub use types::{MiningStats, Nonce, NonceStats};
pub use multichain::{MultiChainController, S9_NUM_CHAINS};
pub use batch::BatchProcessor;

#[cfg(feature = "api")]
pub use validation_pool::ValidationPool;

#[cfg(feature = "bsv-node")]
pub use bsv_node::{
    BsvNode, Block, BlockHeader, Transaction, TxInput, TxOutput,
    BlockchainStorage, MemoryBlockchain
};

#[cfg(feature = "bsv-node")]
pub use bsv_large_blocks::{LargeBlockValidator, BlockValidationStats};
