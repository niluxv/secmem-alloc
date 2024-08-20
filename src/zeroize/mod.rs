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

use crate::macros::precondition_memory_range;

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        // when running miri we chose a pure rust zeroizer
        pub use fallback::zeroize_mem;
    } else if #[cfg(feature = "nightly_core_intrinsics")] {
        pub use nightly::zeroize_mem;
    } else {
        pub use asm_barier::zeroize_mem;
    }
}

/// Very fast nightly-only zeroizer.
///
/// This zeroizer uses the volatile memset intrinsic which does not
/// yet have a stable counterpart. It should be very fast, but requires
/// nightly.
#[cfg(feature = "nightly_core_intrinsics")]
mod nightly {
    use super::*;

    /// Zeroize the memory pointed to by `ptr` and of size `len` bytes.
    ///
    /// This is guarantied to be not elided by the compiler.
    ///
    /// # Safety
    /// The caller *must* ensure that `ptr` is valid for writes of `len` bytes,
    /// see the [`std::ptr`] documentation. In particular this function is
    /// not atomic.
    pub unsafe fn zeroize_mem(ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        // SAFETY: the caller must uphold the safety contract
        unsafe {
            core::intrinsics::volatile_set_memory(ptr, 0, len);
        }
    }
}

/// Fast zeroizer that works on stable and all target platforms.
///
/// This zeroizer uses a non-volatile memset, followed by an empty asm block
/// acting as an optimisation barier. It should be very fast, and according
/// to my current understanding of the op.sem. the compiler is not
/// allowed to remove the writes.
mod asm_barier {
    use super::*;

    /// Zeroize the memory pointed to by `ptr` and of size `len` bytes.
    ///
    /// This is guarantied to be not elided by the compiler.
    ///
    /// # Safety
    /// The caller *must* ensure that `ptr` is valid for writes of `len` bytes,
    /// see the [`std::ptr`] documentation. In particular this function is
    /// not atomic.
    pub unsafe fn zeroize_mem(ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
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

/// Simple zeroize a byte at a time zeroizer, useful for tests and works on
/// miri.
///
/// This zeroizer uses a volatile write per byte. This zeroization technique
/// is pure Rust and available for all target platforms on stable, but very
/// slow.
mod fallback {
    use super::*;

    /// Zeroize the memory pointed to by `ptr` and of size `len` bytes.
    ///
    /// This is guarantied to be not elided by the compiler.
    ///
    /// # Safety
    /// The caller *must* ensure that `ptr` is valid for writes of `len` bytes,
    /// see the [`std::ptr`] documentation. In particular this function is
    /// not atomic.
    pub unsafe fn zeroize_mem(mut ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        for _i in 0..len {
            // SAFETY: `ptr` originally pointed into an allocation of `len` bytes so now,
            // after `_i` steps `len - _i > 0` bytes are left, so `ptr` is valid for
            // a byte write
            unsafe {
                core::ptr::write_volatile(ptr, 0u8);
            }
            // SAFETY: after increment, `ptr` points into the same allocation if `_i == len`
            // or one byte past it, so `add` is sound
            ptr = unsafe { ptr.add(1) };
        }
    }
}

#[cfg(test)]
mod tests;
