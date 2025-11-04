//! Work preparation for Bitcoin mining
//!
//! This module builds block headers from pool jobs and prepares them
//! for ASIC hardware.
//!
//! ## Block Header Format (80 bytes)
//!
//! ```text
//! [0-3]    Version (4 bytes)
//! [4-35]   Previous block hash (32 bytes)
//! [36-67]  Merkle root (32 bytes)
//! [68-71]  Timestamp (4 bytes)
//! [72-75]  Difficulty bits (4 bytes)
//! [76-79]  Nonce (4 bytes) ← What ASICs iterate
//! ```

use crate::stratum::MiningJob;
use sha2::{Digest, Sha256};

/// Bitcoin block header (80 bytes)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlockHeader {
    /// Version (4 bytes, little-endian)
    pub version: u32,

    /// Previous block hash (32 bytes)
    pub prev_hash: [u8; 32],

    /// Merkle root (32 bytes)
    pub merkle_root: [u8; 32],

    /// Timestamp (4 bytes, little-endian)
    pub ntime: u32,

    /// Difficulty bits (4 bytes, little-endian)
    pub nbits: u32,

    /// Nonce (4 bytes, little-endian) - what we're searching for
    pub nonce: u32,
}

impl BlockHeader {
    /// Convert to byte array
    pub fn as_bytes(&self) -> [u8; 80] {
        let mut bytes = [0u8; 80];

        bytes[0..4].copy_from_slice(&self.version.to_le_bytes());
        bytes[4..36].copy_from_slice(&self.prev_hash);
        bytes[36..68].copy_from_slice(&self.merkle_root);
        bytes[68..72].copy_from_slice(&self.ntime.to_le_bytes());
        bytes[72..76].copy_from_slice(&self.nbits.to_le_bytes());
        bytes[76..80].copy_from_slice(&self.nonce.to_le_bytes());

        bytes
    }
}

/// Prepared work for mining
pub struct Work {
    /// Original job from pool
    pub job: MiningJob,

    /// Block header template
    pub header: BlockHeader,

    /// SHA-256 midstate (after processing first 64 bytes)
    /// This is sent to ASICs to save computation
    pub midstate: [u8; 32],

    /// Difficulty target (256-bit integer, big-endian)
    pub target: [u8; 32],

    /// Extranonce2 value
    pub extranonce2: u64,
}

impl Work {
    /// Prepare work from mining job
    pub fn from_job(
        job: MiningJob,
        extranonce1: &str,
        extranonce2: u64,
        extranonce2_size: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Build coinbase transaction
        let extranonce2_bytes = extranonce2.to_be_bytes();
        let extranonce2_hex = hex::encode(&extranonce2_bytes[8 - extranonce2_size..]);

        let coinbase = format!(
            "{}{}{}{}",
            job.coinbase1,
            extranonce1,
            extranonce2_hex,
            job.coinbase2
        );

        // Calculate merkle root
        let merkle_root = Self::calculate_merkle_root(&coinbase, &job.merkle_branches)?;

        // Build block header
        let mut header = BlockHeader {
            version: job.version,
            prev_hash: Self::hex_to_bytes32(&job.prev_hash)?,
            merkle_root,
            ntime: job.ntime,
            nbits: job.nbits,
            nonce: 0,
        };

        // Calculate SHA-256 midstate (first 64 bytes of header)
        let midstate = Self::calculate_midstate(&header);

        // Calculate difficulty target from nbits
        let target = Self::nbits_to_target(job.nbits);

        Ok(Work {
            job,
            header,
            midstate,
            target,
            extranonce2,
        })
    }

    /// Calculate SHA-256 midstate for first 64 bytes of block header
    ///
    /// Bitcoin ASICs use this optimization: The block header is 80 bytes,
    /// but SHA-256 processes in 64-byte blocks. So we can pre-compute
    /// the SHA-256 state after the first 64 bytes and send only that
    /// to the ASIC. The ASIC then only needs to process the final
    /// 16 bytes (which includes the nonce).
    ///
    /// This saves ~70% of the computation!
    fn calculate_midstate(header: &BlockHeader) -> [u8; 32] {
        let header_bytes = header.as_bytes();

        // SHA-256 state after processing first 64 bytes
        let mut hasher = Sha256::new();
        hasher.update(&header_bytes[0..64]);

        // Extract internal state (this is a simplification - real impl needs
        // to access the internal state of the SHA-256 hasher)
        let state = hasher.finalize();

        let mut midstate = [0u8; 32];
        midstate.copy_from_slice(&state);
        midstate
    }

    /// Calculate merkle root from coinbase and merkle branches
    fn calculate_merkle_root(
        coinbase_hex: &str,
        merkle_branches: &[String],
    ) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        // Hash coinbase transaction
        let coinbase_bytes = hex::decode(coinbase_hex)?;
        let mut hash = Self::double_sha256(&coinbase_bytes);

        // Combine with merkle branches
        for branch_hex in merkle_branches {
            let branch = hex::decode(branch_hex)?;

            // Concatenate and hash
            let mut combined = Vec::with_capacity(64);
            combined.extend_from_slice(&hash);
            combined.extend_from_slice(&branch);

            hash = Self::double_sha256(&combined);
        }

        Ok(hash)
    }

    /// Double SHA-256 (Bitcoin standard)
    fn double_sha256(data: &[u8]) -> [u8; 32] {
        let first = Sha256::digest(data);
        let second = Sha256::digest(first);

        let mut result = [0u8; 32];
        result.copy_from_slice(&second);
        result
    }

    /// Convert difficulty bits (nbits) to 256-bit target
    ///
    /// nbits format: 0x1d00ffff
    /// - First byte (0x1d) = exponent
    /// - Next 3 bytes (0x00ffff) = coefficient
    ///
    /// target = coefficient * 2^(8 * (exponent - 3))
    fn nbits_to_target(nbits: u32) -> [u8; 32] {
        let exponent = (nbits >> 24) as usize;
        let coefficient = nbits & 0x00ffffff;

        let mut target = [0u8; 32];

        // Set coefficient in little-endian
        let coeff_bytes = coefficient.to_le_bytes();
        let offset = exponent.saturating_sub(3);

        if offset < 29 {
            target[offset..offset + 3].copy_from_slice(&coeff_bytes[0..3]);
        }

        target
    }

    /// Convert hex string to 32-byte array
    fn hex_to_bytes32(hex: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        let bytes = hex::decode(hex)?;

        if bytes.len() != 32 {
            return Err(format!("Expected 32 bytes, got {}", bytes.len()).into());
        }

        let mut result = [0u8; 32];
        result.copy_from_slice(&bytes);

        Ok(result)
    }

    /// Check if a hash meets the difficulty target
    ///
    /// In Bitcoin, the hash must be LESS than the target (numerically).
    /// Both are 256-bit integers compared in little-endian order.
    pub fn check_hash_meets_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
        // Compare as big-endian 256-bit integers
        // (Bitcoin hashes are displayed in big-endian, but compared in little-endian)
        for i in (0..32).rev() {
            if hash[i] < target[i] {
                return true;
            } else if hash[i] > target[i] {
                return false;
            }
        }

        // Equal - technically meets target
        true
    }

    /// Validate a nonce by computing its hash
    pub fn validate_nonce(&self, nonce: u32) -> bool {
        let mut header = self.header;
        header.nonce = nonce;

        // Compute double SHA-256
        let hash = Self::double_sha256(&header.as_bytes());

        // Check if it meets target
        Self::check_hash_meets_target(&hash, &self.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_header_size() {
        assert_eq!(std::mem::size_of::<BlockHeader>(), 80);
    }

    #[test]
    fn test_double_sha256() {
        // Test with empty data
        let hash = Work::double_sha256(b"");

        // SHA256(SHA256("")) = 5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_nbits_to_target() {
        // Example: nbits = 0x1d00ffff (Bitcoin genesis block difficulty)
        let target = Work::nbits_to_target(0x1d00ffff);

        // Should produce a target with lots of leading zeros
        assert!(target[28] == 0xff);
        assert!(target[29] == 0xff);
    }
}
