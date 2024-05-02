//! Functions for securely wiping memory.
//!
//! This module contains functions for securely and efficiently zeroizing
//! memory. These operations won't be optimized away be the compiler. They
//! operate on raw memory regions and can invalidate the memory for types that
//! do not have all zeros (binary) as a valid value. They should be used during
//! deallocation, because the memory is unused and the memory needs not contain
//! a value of a certain type than.
//!
//! For good general purpose memory wiping use the [`zeroize`](https://crates.io/crates/zeroize)
//! crate.

use crate::internals::zeroize as internals;
use crate::macros::{
    debug_precondition_logaligned, debug_precondition_logmultiple, precondition_memory_range,
};
use crate::util::is_aligned_ptr_mut;

/// Strategy for securely erasing memory.
///
/// # Security
/// The implementor *must* ensure that the zeroize instruction won't be elided
/// by the compiler.
pub trait MemZeroizer {
    /// Zeroize the memory pointed to by `ptr` and of size `len` bytes.
    ///
    /// This is guarantied to be not elided by the compiler.
    ///
    /// # Safety
    /// The caller *must* ensure that `ptr` is valid for writes of `len` bytes,
    /// see the [`std::ptr`] documentation. In particular this function is
    /// not atomic.
    ///
    /// Furthermore, `ptr` *must* be at least `2^LOG_ALIGN` byte aligned, and
    /// `2^LOG_ALIGN` must fit a `usize`.
    ///
    /// Finally `len` must be a multiple of `2^LOG_MULTIPLE`, and `2^LOG_ALIGN`
    /// must fit a `usize`. (If `len` is not a multiple of `2^LOG_MULTIPLE`
    /// then this won't result in UB but the memory pointed to by `ptr` might
    /// only be zeroized for `len` rounded down to a multiple `2^LOG_MULTIPLE`
    /// bytes, or the full `len` bytes, or anything in between, or the function
    /// might panic.)
    unsafe fn zeroize_mem_blocks<const LOG_ALIGN: u8, const LOG_MULTIPLE: u8>(
        &self,
        ptr: *mut u8,
        len: usize,
    );

    /// Zeroize the memory pointed to by `ptr` and of size `len` bytes.
    /// Shorthand for `Self::zeroize_mem_blocks::<0, 0>`.
    ///
    /// This is guarantied to be not elided by the compiler.
    ///
    /// # Safety
    /// The caller *must* ensure that `ptr` is valid for writes of `len` bytes,
    /// see the [`std::ptr`] documentation. In particular this function is
    /// not atomic.
    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize) {
        unsafe { self.zeroize_mem_blocks::<0, 0>(ptr, len) }
    }
}

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        // when running miri we chose a pure rust zeroizer by default
        pub type DefaultMemZeroizer = VolatileWrite8Zeroizer;
        pub(crate) use VolatileWrite8Zeroizer as DefaultMemZeroizerConstructor;
    } else if #[cfg(feature = "nightly_core_intrinsics")] {
        /// Best (i.e. fastest) [`MemZeroizer`] available for the target.
        ///
        /// Which [`MemZeroizer`] this is is an implementation detail, can depend on the target and
        /// the selected features and the version of this library.
        pub type DefaultMemZeroizer = VolatileMemsetZeroizer;
        pub(crate) use VolatileMemsetZeroizer as DefaultMemZeroizerConstructor;
    } else {
        pub type DefaultMemZeroizer = MemsetAsmBarierZeroizer;
        pub(crate) use MemsetAsmBarierZeroizer as DefaultMemZeroizerConstructor;
    }
}

#[cfg(test)]
pub(crate) use VolatileWrite8Zeroizer as TestZeroizer;

/// This zeroizer uses the volatile memset intrinsic which does not
/// yet have a stable counterpart. It should be very fast, but requires
/// nightly.
#[cfg(feature = "nightly_core_intrinsics")]
#[derive(Debug, Copy, Clone, Default)]
pub struct VolatileMemsetZeroizer;

#[cfg(feature = "nightly_core_intrinsics")]
impl MemZeroizer for VolatileMemsetZeroizer {
    unsafe fn zeroize_mem_blocks<const A: u8, const B: u8>(&self, ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // SAFETY: the caller must uphold the safety contract
        unsafe {
            core::intrinsics::volatile_set_memory(ptr, 0, len);
        }
    }
}

/// This zeroizer uses a non-volatile memset, followed by an empty asm block
/// acting as an optimisation barier. It should be very fast, and according to
/// my current understanding of the op.sem. the compiler is not allowed to
/// remove the writes.
#[derive(Debug, Copy, Clone, Default)]
pub struct MemsetAsmBarierZeroizer;

impl MemZeroizer for MemsetAsmBarierZeroizer {
    unsafe fn zeroize_mem_blocks<const A: u8, const B: u8>(&self, ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // SAFETY: the caller must uphold the safety contract of `write_bytes`
        unsafe { ptr.write_bytes(0, len) };
        // Optimisation barier, so the writes can not be optimised out
        unsafe {
            core::arch::asm!(
                "/* {0} */",
                in(reg) ptr,
                options(nostack, readonly, preserves_flags),
            )
        };
    }
}

/// This zeroizer uses a volatile write per 8 bytes if the pointer is 8 byte
/// aligned, and otherwise uses `VolatileWriteZeroizer`. This zeroization
/// technique is pure Rust and available for all target platforms on stable, but
/// not very fast.
///
/// This zeroization method can benefit (in terms of performance) from using the
/// [`MemZeroizer::zeroize_mem_blocks`] function instead of
/// [`MemZeroizer::zeroize_mem`] function if a minimum alignment is known
/// at compile time.
#[derive(Debug, Copy, Clone, Default)]
pub struct VolatileWrite8Zeroizer;

impl MemZeroizer for VolatileWrite8Zeroizer {
    unsafe fn zeroize_mem_blocks<const A: u8, const B: u8>(&self, mut ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        debug_precondition_logmultiple!(B, len);
        // if we have 8 = 2^3 byte alignment then write 8 bytes at a time,
        // otherwise byte-for-byte
        if (A >= 3) | is_aligned_ptr_mut(ptr, 8) {
            // SAFETY: by the above check, `ptr` is at least 8 byte aligned
            // SAFETY: the other safety requirements uphold by caller
            ptr = unsafe { internals::zeroize_align8_block8(ptr, len) };
            if B < 3 {
                unsafe { internals::zeroize_align4_tail8(ptr, len) };
            }
        } else {
            // SAFETY: the caller must uphold the contract of `volatile_write_zeroize_mem`
            unsafe {
                internals::volatile_write_zeroize(ptr, len);
            }
        }
    }
}

#[cfg(test)]
mod tests;
