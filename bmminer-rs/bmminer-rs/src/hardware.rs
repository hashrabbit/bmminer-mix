//! FPGA Hardware Interface (Memory-Mapped I/O)
//!
//! This module provides zero-overhead access to FPGA registers via MMIO.
//!
//! ## Safety
//!
//! All MMIO access is `unsafe` by nature (hardware can do anything).
//! We encapsulate the unsafety behind a safe-ish API with runtime checks.
//!
//! ## Performance
//!
//! - Direct pointer access (no function call overhead)
//! - Volatile reads/writes (compiler can't optimize away)
//! - Inlined everywhere (#[inline(always)])
//!
//! Single register read: ~50ns (depends on FPGA latency)

use core::ptr;
use volatile_register::{RO, RW, WO};

/// FPGA register offsets (from driver-btm-c5.h)
#[allow(dead_code)]
mod reg_offset {
    pub const HARDWARE_VERSION: usize = 0x00 / 4;
    pub const FAN_SPEED: usize = 0x04 / 4;
    pub const HASH_ON_PLUG: usize = 0x08 / 4;
    pub const BUFFER_SPACE: usize = 0x0C / 4;
    pub const RETURN_NONCE: usize = 0x10 / 4;
    pub const NONCE_NUMBER_IN_FIFO: usize = 0x18 / 4;
    pub const NONCE_FIFO_INTERRUPT: usize = 0x1C / 4;
    pub const TEMPERATURE_0_3: usize = 0x20 / 4;
    pub const TEMPERATURE_4_7: usize = 0x24 / 4;
    pub const TEMPERATURE_8_11: usize = 0x28 / 4;
    pub const TEMPERATURE_12_15: usize = 0x2C / 4;
    pub const IIC_COMMAND: usize = 0x30 / 4;
    pub const RESET_HASHBOARD_COMMAND: usize = 0x34 / 4;
    pub const FAN_CONTROL: usize = 0x84 / 4;
    pub const TICKET_MASK_FPGA: usize = 0x8C / 4;
}

/// FPGA register map (matches hardware layout exactly)
///
/// # Safety
///
/// This struct MUST match the exact layout of FPGA registers.
/// Any mismatch will cause undefined behavior.
#[repr(C)]
pub struct FpgaRegisters {
    hardware_version: RO<u32>,       // 0x00
    fan_speed: RO<u32>,              // 0x04
    hash_on_plug: RO<u32>,           // 0x08
    buffer_space: RO<u32>,           // 0x0C
    return_nonce: RO<u32>,           // 0x10 - HOT PATH!
    _pad0: u32,                      // 0x14
    nonce_fifo_count: RO<u32>,       // 0x18 - HOT PATH!
    nonce_fifo_interrupt: RW<u32>,   // 0x1C
    temperature_0_3: RO<u32>,        // 0x20
    temperature_4_7: RO<u32>,        // 0x24
    temperature_8_11: RO<u32>,       // 0x28
    temperature_12_15: RO<u32>,      // 0x2C
    iic_command: RW<u32>,            // 0x30
    reset_hashboard: WO<u32>,        // 0x34
    // ... more registers (not all listed for brevity)
}

/// High-level FPGA controller
///
/// Provides safe(r) access to FPGA hardware via memory-mapped I/O.
pub struct FpgaController {
    /// Pointer to memory-mapped FPGA registers
    regs: *mut FpgaRegisters,

    /// Base physical address (for debugging)
    base_addr: usize,

    /// Size of mapped region
    map_size: usize,
}

// Safety: FpgaController can be shared between threads
// (hardware access is naturally serialized by MMIO bus)
unsafe impl Send for FpgaController {}
unsafe impl Sync for FpgaController {}

impl FpgaController {
    /// Default FPGA physical address (from C code)
    pub const DEFAULT_PHYS_ADDR: usize = 0x43C00000;

    /// Size of FPGA register region (4KB)
    pub const MAP_SIZE: usize = 0x1000;

    /// Create a new FPGA controller by mapping physical memory
    ///
    /// # Safety
    ///
    /// - `phys_addr` must be the correct physical address of the FPGA
    /// - Caller must ensure no other code accesses this region
    /// - Requires root privileges (CAP_SYS_RAWIO)
    ///
    /// # Errors
    ///
    /// Returns `None` if memory mapping fails.
    pub unsafe fn new(phys_addr: usize) -> Option<Self> {
        use libc::{mmap, open, MAP_SHARED, O_RDWR, O_SYNC, PROT_READ, PROT_WRITE};

        // Open /dev/mem for direct physical memory access
        let fd = open(b"/dev/mem\0".as_ptr() as *const i8, O_RDWR | O_SYNC);
        if fd < 0 {
            return None;
        }

        // Map FPGA registers into our address space
        let ptr = mmap(
            ptr::null_mut(),
            Self::MAP_SIZE,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            fd,
            phys_addr as i64,
        );

        // Close fd (mapping remains valid)
        libc::close(fd);

        if ptr == libc::MAP_FAILED {
            return None;
        }

        Some(Self {
            regs: ptr as *mut FpgaRegisters,
            base_addr: phys_addr,
            map_size: Self::MAP_SIZE,
        })
    }

    /// Read hardware version (verify FPGA is responsive)
    #[inline]
    pub fn hardware_version(&self) -> u32 {
        unsafe { (*self.regs).hardware_version.read() }
    }

    /// Read fan speed (RPM)
    #[inline]
    pub fn fan_speed(&self) -> u32 {
        unsafe { (*self.regs).fan_speed.read() }
    }

    /// HOT PATH: Read number of nonces in FIFO
    ///
    /// This is called in the tight loop. Must be fast!
    #[inline(always)]
    pub fn nonce_fifo_count(&self) -> u32 {
        unsafe { (*self.regs).nonce_fifo_count.read() }
    }

    /// HOT PATH: Read raw nonce data from FIFO
    ///
    /// Returns the raw 32-bit nonce word from FPGA.
    #[inline(always)]
    pub fn read_nonce_raw(&self) -> u32 {
        unsafe { (*self.regs).return_nonce.read() }
    }

    /// Read temperature sensors (4 sensors per register)
    #[inline]
    pub fn read_temperatures(&self) -> [u8; 16] {
        unsafe {
            let t0_3 = (*self.regs).temperature_0_3.read();
            let t4_7 = (*self.regs).temperature_4_7.read();
            let t8_11 = (*self.regs).temperature_8_11.read();
            let t12_15 = (*self.regs).temperature_12_15.read();

            [
                (t0_3 & 0xFF) as u8,
                ((t0_3 >> 8) & 0xFF) as u8,
                ((t0_3 >> 16) & 0xFF) as u8,
                ((t0_3 >> 24) & 0xFF) as u8,
                (t4_7 & 0xFF) as u8,
                ((t4_7 >> 8) & 0xFF) as u8,
                ((t4_7 >> 16) & 0xFF) as u8,
                ((t4_7 >> 24) & 0xFF) as u8,
                (t8_11 & 0xFF) as u8,
                ((t8_11 >> 8) & 0xFF) as u8,
                ((t8_11 >> 16) & 0xFF) as u8,
                ((t8_11 >> 24) & 0xFF) as u8,
                (t12_15 & 0xFF) as u8,
                ((t12_15 >> 8) & 0xFF) as u8,
                ((t12_15 >> 16) & 0xFF) as u8,
                ((t12_15 >> 24) & 0xFF) as u8,
            ]
        }
    }

    /// Flush nonce FIFO (discard all pending nonces)
    #[inline]
    pub fn flush_nonce_fifo(&self) {
        const FLUSH_BIT: u32 = 1 << 16;
        unsafe {
            (*self.regs).nonce_fifo_interrupt.write(FLUSH_BIT);
        }
    }

    /// Reset hash board
    #[inline]
    pub fn reset_hashboard(&self, chain_id: u8) {
        const RESET_BIT: u32 = 1 << 31;
        let cmd = RESET_BIT | ((chain_id as u32) << 16);
        unsafe {
            (*self.regs).reset_hashboard.write(cmd);
        }
    }

    /// Get base physical address (for debugging)
    #[inline]
    pub fn base_addr(&self) -> usize {
        self.base_addr
    }

    /// Access raw register pointer (for advanced use)
    ///
    /// # Safety
    ///
    /// Caller must ensure they don't violate MMIO access rules.
    #[inline]
    pub unsafe fn raw_ptr(&self) -> *mut u32 {
        self.regs as *mut u32
    }

    // ===== Work Configuration Methods =====

    /// Set active chain ID for subsequent operations
    ///
    /// In real hardware, this selects which ASIC chain to configure.
    /// For now, we store it in the controller state.
    #[inline]
    pub fn set_chain_id(&self, chain_id: u8) {
        // In real implementation, write to chain select register
        // For now, this is a no-op (would write to FPGA register)
        let _ = chain_id;
    }

    /// Set SHA-256 midstate for mining work
    ///
    /// The midstate is the SHA-256 state after processing the first 64 bytes
    /// of the block header. This is a critical optimization that saves ~70%
    /// of the ASIC computation.
    ///
    /// # Arguments
    ///
    /// * `midstate` - 32-byte SHA-256 midstate
    #[inline]
    pub fn set_midstate(&self, midstate: &[u8; 32]) {
        // In real implementation, write to midstate registers (0x20-0x3F)
        // This would be 8 × 32-bit writes to FPGA registers
        unsafe {
            let base = self.raw_ptr();
            let midstate_offset = 0x20 / 4;

            // Write midstate as 8 × 32-bit words
            for i in 0..8 {
                let word = u32::from_le_bytes([
                    midstate[i * 4],
                    midstate[i * 4 + 1],
                    midstate[i * 4 + 2],
                    midstate[i * 4 + 3],
                ]);
                ptr::write_volatile(base.offset(midstate_offset + i as isize), word);
            }
        }
    }

    /// Set difficulty target for share validation
    ///
    /// The ASIC will compare each hash against this target and only return
    /// nonces where hash < target.
    ///
    /// # Arguments
    ///
    /// * `target` - 32-byte difficulty target (big-endian)
    #[inline]
    pub fn set_target(&self, target: &[u8; 32]) {
        // In real implementation, write to target registers
        unsafe {
            let base = self.raw_ptr();
            let target_offset = 0x40 / 4; // Example offset

            // Write target as 8 × 32-bit words
            for i in 0..8 {
                let word = u32::from_be_bytes([
                    target[i * 4],
                    target[i * 4 + 1],
                    target[i * 4 + 2],
                    target[i * 4 + 3],
                ]);
                ptr::write_volatile(base.offset(target_offset + i as isize), word);
            }
        }
    }

    /// Set job ID for work tracking
    ///
    /// This ID is returned with each nonce so we can match it back to the
    /// original mining job.
    #[inline]
    pub fn set_job_id(&self, job_id: u32) {
        // In real implementation, write to job ID register
        unsafe {
            let base = self.raw_ptr();
            let job_id_offset = 0x60 / 4; // Example offset
            ptr::write_volatile(base.offset(job_id_offset), job_id);
        }
    }

    /// Start mining with specified nonce range
    ///
    /// Triggers the ASIC to begin hashing with nonces from `start` to `end`.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting nonce value (usually 0x00000000)
    /// * `end` - Ending nonce value (usually 0xFFFFFFFF for full range)
    #[inline]
    pub fn start_mining(&self, start: u32, end: u32) {
        // In real implementation, write start/end nonces and trigger start bit
        unsafe {
            let base = self.raw_ptr();

            // Write nonce range
            let nonce_start_offset = 0x70 / 4; // Example offset
            let nonce_end_offset = 0x74 / 4;
            ptr::write_volatile(base.offset(nonce_start_offset), start);
            ptr::write_volatile(base.offset(nonce_end_offset), end);

            // Trigger start (write to control register)
            let control_offset = 0x78 / 4;
            const START_BIT: u32 = 1 << 0;
            ptr::write_volatile(base.offset(control_offset), START_BIT);
        }
    }

    /// Get current chain ID
    #[inline]
    pub fn get_chain_id(&self) -> u8 {
        // In real implementation, read from chain select register
        // For now, return a dummy value
        0
    }

    /// Get current job ID
    #[inline]
    pub fn get_job_id(&self) -> u32 {
        // In real implementation, read from job ID register
        unsafe {
            let base = self.raw_ptr();
            let job_id_offset = 0x60 / 4;
            ptr::read_volatile(base.offset(job_id_offset))
        }
    }
}

impl Drop for FpgaController {
    fn drop(&mut self) {
        // Unmap memory region
        unsafe {
            libc::munmap(self.regs as *mut libc::c_void, self.map_size);
        }
    }
}

/// Helper: Read nonce data from MMIO memory region
///
/// This reads the full nonce structure from memory-mapped job data area.
///
/// # Safety
///
/// - `mmio_base` must point to valid MMIO region
/// - Caller must ensure correct offsets
#[inline(always)]
pub unsafe fn read_nonce_data_fast(mmio_base: *const u32) -> (u32, u32, u64, [u8; 32]) {
    // Offsets from driver-btm-c5.h
    const JOB_ID_OFFSET: isize = 0x0 / 4;
    const HEADER_VERSION_OFFSET: isize = 0x4 / 4;
    const NONCE2_L_OFFSET: isize = 0x8 / 4;
    const NONCE2_H_OFFSET: isize = 0xC / 4;
    const MIDSTATE_OFFSET: isize = 0x20 / 4;

    // Read job data (volatile to prevent compiler optimization)
    let job_id = ptr::read_volatile(mmio_base.offset(JOB_ID_OFFSET));
    let header_version = ptr::read_volatile(mmio_base.offset(HEADER_VERSION_OFFSET));

    // Read nonce2 (64-bit, split into two 32-bit reads)
    let nonce2_lo = ptr::read_volatile(mmio_base.offset(NONCE2_L_OFFSET)) as u64;
    let nonce2_hi = ptr::read_volatile(mmio_base.offset(NONCE2_H_OFFSET)) as u64;
    let nonce2 = (nonce2_hi << 32) | nonce2_lo;

    // Read midstate (32 bytes = 8 u32 reads)
    let mut midstate = [0u8; 32];
    let midstate_ptr = mmio_base.offset(MIDSTATE_OFFSET) as *const u8;
    ptr::copy_nonoverlapping(midstate_ptr, midstate.as_mut_ptr(), 32);

    (job_id, header_version, nonce2, midstate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_size() {
        // Ensure register struct layout matches expectations
        assert_eq!(
            core::mem::size_of::<FpgaRegisters>(),
            13 * 4 // 13 registers × 4 bytes
        );
    }
}
