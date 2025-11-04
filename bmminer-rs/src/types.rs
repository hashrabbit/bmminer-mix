//! Core types for mining operations
//!
//! All types are carefully designed for:
//! - Cache alignment (avoid false sharing)
//! - Zero-copy operations
//! - Efficient memory layout

use core::fmt;

/// A nonce returned from the FPGA
///
/// Layout matches FPGA MMIO registers for zero-copy reads.
/// Aligned to 128 bytes (2 cache lines) to prevent false sharing.
#[repr(C, align(128))]
#[derive(Clone, Copy)]
pub struct Nonce {
    /// Work ID (from FPGA)
    pub work_id: u32,

    /// Nonce value (candidate solution)
    pub nonce3: u32,

    /// Which ASIC chain produced this nonce
    pub chain_num: u8,

    /// Padding for alignment
    _pad: [u8; 3],

    /// Job ID from pool
    pub job_id: u32,

    /// Block header version
    pub header_version: u32,

    /// Extranonce2 value
    pub nonce2: u64,

    /// SHA-256 midstate (32 bytes)
    pub midstate: [u8; 32],

    /// Timestamp when nonce was received (for latency tracking)
    pub timestamp_ns: u64,

    /// Padding to fill cache line
    _pad2: [u8; 48],
}

impl Nonce {
    /// Create a new nonce (for testing)
    #[inline]
    pub const fn new() -> Self {
        Self {
            work_id: 0,
            nonce3: 0,
            chain_num: 0,
            _pad: [0; 3],
            job_id: 0,
            header_version: 0,
            nonce2: 0,
            midstate: [0; 32],
            timestamp_ns: 0,
            _pad2: [0; 48],
        }
    }

    /// Check if this nonce is valid (basic sanity check)
    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        // Chain number must be in valid range (0-15)
        self.chain_num < 16
    }
}

impl Default for Nonce {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Nonce")
            .field("work_id", &self.work_id)
            .field("nonce3", &format_args!("0x{:08x}", self.nonce3))
            .field("chain_num", &self.chain_num)
            .field("job_id", &self.job_id)
            .field("nonce2", &format_args!("0x{:016x}", self.nonce2))
            .finish()
    }
}

/// Statistics for nonce processing
#[repr(C, align(64))]
#[derive(Default)]
pub struct NonceStats {
    /// Total nonces received
    pub total_received: u64,

    /// Nonces dropped due to queue full
    pub dropped: u64,

    /// Invalid nonces
    pub invalid: u64,

    /// Valid shares submitted
    pub shares_submitted: u64,

    /// Shares accepted by pool
    pub shares_accepted: u64,

    /// Shares rejected by pool
    pub shares_rejected: u64,

    /// Average processing latency (nanoseconds)
    pub avg_latency_ns: u64,

    /// Peak latency (nanoseconds)
    pub peak_latency_ns: u64,
}

/// Thread-safe mining statistics using atomics
///
/// Designed for concurrent access from multiple threads without locks.
/// Each counter is on its own cache line to prevent false sharing.
#[repr(C, align(128))]
pub struct MiningStats {
    /// Number of mining jobs received from pool
    pub jobs_received: std::sync::atomic::AtomicU64,
    _pad1: [u8; 120],

    /// Number of nonces collected from FPGA
    pub nonces_collected: std::sync::atomic::AtomicU64,
    _pad2: [u8; 120],

    /// Number of nonces processed (validated)
    pub nonces_processed: std::sync::atomic::AtomicU64,
    _pad3: [u8; 120],

    /// Number of nonces dropped (queue full)
    pub nonces_dropped: std::sync::atomic::AtomicU64,
    _pad4: [u8; 120],

    /// Number of valid shares found
    pub shares_found: std::sync::atomic::AtomicU64,
    _pad5: [u8; 120],

    /// Number of shares submitted to pool
    pub shares_submitted: std::sync::atomic::AtomicU64,
    _pad6: [u8; 120],

    /// Number of shares accepted by pool
    pub shares_accepted: std::sync::atomic::AtomicU64,
    _pad7: [u8; 120],

    /// Number of shares rejected by pool
    pub shares_rejected: std::sync::atomic::AtomicU64,
    _pad8: [u8; 120],
}

impl MiningStats {
    /// Create a new statistics tracker
    pub fn new() -> Self {
        Self {
            jobs_received: std::sync::atomic::AtomicU64::new(0),
            _pad1: [0; 120],
            nonces_collected: std::sync::atomic::AtomicU64::new(0),
            _pad2: [0; 120],
            nonces_processed: std::sync::atomic::AtomicU64::new(0),
            _pad3: [0; 120],
            nonces_dropped: std::sync::atomic::AtomicU64::new(0),
            _pad4: [0; 120],
            shares_found: std::sync::atomic::AtomicU64::new(0),
            _pad5: [0; 120],
            shares_submitted: std::sync::atomic::AtomicU64::new(0),
            _pad6: [0; 120],
            shares_accepted: std::sync::atomic::AtomicU64::new(0),
            _pad7: [0; 120],
            shares_rejected: std::sync::atomic::AtomicU64::new(0),
            _pad8: [0; 120],
        }
    }

    /// Reset all counters to zero
    pub fn reset(&self) {
        use std::sync::atomic::Ordering;
        self.jobs_received.store(0, Ordering::Relaxed);
        self.nonces_collected.store(0, Ordering::Relaxed);
        self.nonces_processed.store(0, Ordering::Relaxed);
        self.nonces_dropped.store(0, Ordering::Relaxed);
        self.shares_found.store(0, Ordering::Relaxed);
        self.shares_submitted.store(0, Ordering::Relaxed);
        self.shares_accepted.store(0, Ordering::Relaxed);
        self.shares_rejected.store(0, Ordering::Relaxed);
    }
}

impl Default for MiningStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_size_alignment() {
        // Verify nonce is 128 bytes (2 cache lines)
        assert_eq!(core::mem::size_of::<Nonce>(), 128);
        assert_eq!(core::mem::align_of::<Nonce>(), 128);
    }

    #[test]
    fn nonce_stats_alignment() {
        // Verify stats is 64 bytes (1 cache line)
        assert_eq!(core::mem::align_of::<NonceStats>(), 64);
    }
}
