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
use crate::macros::{debug_precondition_logaligned, precondition_memory_range};

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
    /// Furthermore, `ptr` *must* be at least `2^ALIGN` byte aligned, and
    /// `2^ALIGN` must fit a `usize`.
    /// must be a power of 2 (and therefore non-zero).
    unsafe fn zeroize_mem_aligned<const LOG_ALIGN: u8>(&self, ptr: *mut u8, len: usize);

    /// Zeroize the memory pointed to by `ptr` and of size `len` bytes.
    /// Shorthand for `Self::zeroize_mem_aligned::<0>`.
    ///
    /// This is guarantied to be not elided by the compiler.
    ///
    /// # Safety
    /// The caller *must* ensure that `ptr` is valid for writes of `len` bytes,
    /// see the [`std::ptr`] documentation. In particular this function is
    /// not atomic.
    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize) {
        unsafe { self.zeroize_mem_aligned::<0>(ptr, len) }
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
    } else if #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "macos",
        target_os = "ios",
        target_env = "gnu",
        target_env = "musl"
    ))] {
        /// Best (i.e. fastest) [`MemZeroizer`] available for the target.
        ///
        /// Which [`MemZeroizer`] this is is an implementation detail, can depend on the target and
        /// the selected features and the version of this library.
        pub type DefaultMemZeroizer = LibcZeroizer;
        pub(crate) use LibcZeroizer as DefaultMemZeroizerConstructor;
    } else {
        /// Best (i.e. fastest) [`MemZeroizer`] available for the target.
        ///
        /// Which [`MemZeroizer`] this is is an implementation detail, can depend on the target and
        /// the selected features and the version of this library.
        pub type DefaultMemZeroizer = VolatileWrite8Zeroizer;
        pub(crate) use VolatileWrite8Zeroizer as DefaultMemZeroizerConstructor;
    }
}

#[cfg(test)]
pub(crate) use VolatileWrite8Zeroizer as TestZeroizer;

/// This zeroizer uses the volatile memset intrinsic which does not
/// yet have a stable counterpart. It should be very fast, but requires
/// nightly.
///
/// In addition to the volatile write we place a compiler fence right next to
/// the volatile write. This should not be necessary for secure zeroization
/// since the volatile semantics guarenties our writes are not elided, and they
/// can not be delayed since we are deallocating the memory after zeroization.
/// The use of this fence is therefore only a precaution.
#[cfg(feature = "nightly_core_intrinsics")]
#[derive(Debug, Copy, Clone, Default)]
pub struct VolatileMemsetZeroizer;

#[cfg(feature = "nightly_core_intrinsics")]
impl MemZeroizer for VolatileMemsetZeroizer {
    unsafe fn zeroize_mem_aligned<const A: u8>(&self, ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // SAFETY: the caller must uphold the safety contract of
        // `internals::volatile_memset`
        unsafe {
            internals::volatile_memset(ptr, 0, len);
        }
        fence();
    }
}

/// This zeroizer uses volatile zeroization functions provided by libc.
/// It should be fast but is only available on certain platforms.
///
/// In addition to the volatile write we place a compiler fence right next to
/// the volatile write. This should not be necessary for secure zeroization
/// since the volatile semantics guarenties our writes are not elided, and they
/// can not be delayed since we are deallocating the memory after zeroization.
/// The use of this fence is therefore only a precaution.
#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "macos",
    target_os = "ios",
    target_env = "gnu",
    target_env = "musl"
))]
#[derive(Debug, Copy, Clone, Default)]
pub struct LibcZeroizer;

#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "macos",
    target_os = "ios",
    target_env = "gnu",
    target_env = "musl"
))]
impl MemZeroizer for LibcZeroizer {
    unsafe fn zeroize_mem_aligned<const A: u8>(&self, ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // SAFETY: the caller must uphold the safety contract of
        // `internals::libc_explicit_bzero`
        unsafe {
            internals::libc_explicit_bzero(ptr, len);
        }
        fence();
    }
}

/// This zeroizer uses volatile assembly (`rep stosb`) for modern x86_64,
/// performing very well for large amounts of memory. To make this available on
/// stable, it uses a C compiler at build time.
///
/// In addition to the volatile write we place a compiler fence right next to
/// the volatile write. This should not be necessary for secure zeroization
/// since the volatile semantics guarenties our writes are not elided, and they
/// can not be delayed since we are deallocating the memory after zeroization.
/// The use of this fence is therefore only a precaution.
#[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
#[derive(Debug, Copy, Clone, Default)]
pub struct AsmRepStosZeroizer;

#[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
impl MemZeroizer for AsmRepStosZeroizer {
    unsafe fn zeroize_mem_aligned<const A: u8>(&self, ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // SAFETY: the caller must uphold the safety contract of
        // `internals::asm_ermsb_zeroize`
        unsafe {
            internals::asm_ermsb_zeroize(ptr, len);
        }
        fence();
    }
}

/// This zeroizer uses a volatile write per byte. This zeroization technique is
/// similar to the `zeroize` crate, available for all target platforms on
/// stable, but extremely slow.
///
/// In addition to the volatile write we place a compiler fence right next to
/// the volatile write. This should not be necessary for secure zeroization
/// since the volatile semantics guarenties our writes are not elided, and they
/// can not be delayed since we are deallocating the memory after zeroization.
/// The use of this fence is therefore only a precaution.
#[derive(Debug, Copy, Clone, Default)]
pub struct VolatileWriteZeroizer;

impl MemZeroizer for VolatileWriteZeroizer {
    unsafe fn zeroize_mem_aligned<const A: u8>(&self, ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // SAFETY: the caller must uphold the safety contract of
        // `volatile_write_zeroize_mem`
        unsafe {
            internals::volatile_write_zeroize(ptr, len);
        }
        fence();
    }
}

/// This zeroizer uses a volatile write per 8 bytes if the pointer is 8 byte
/// aligned, and otherwise uses `VolatileWriteZeroizer`. This zeroization
/// technique is available for all target platforms on stable, but not very
/// fast.
///
/// In addition to the volatile write we place a compiler fence right next to
/// the volatile write. This should not be necessary for secure zeroization
/// since the volatile semantics guarenties our writes are not elided, and they
/// can not be delayed since we are deallocating the memory after zeroization.
/// The use of this fence is therefore only a precaution.
///
/// This zeroization method can benefit (in terms of performance) from using the
/// [`MemZeroizer::zeroize_mem_aligned`] function instead of
/// [`MemZeroizer::zeroize_mem`] function if a minimum alignment is known
/// at compile time.
#[derive(Debug, Copy, Clone, Default)]
pub struct VolatileWrite8Zeroizer;

impl MemZeroizer for VolatileWrite8Zeroizer {
    unsafe fn zeroize_mem_aligned<const A: u8>(&self, ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // if we have 8 = 2^3 byte alignment then write 8 bytes at a time,
        // otherwise byte-for-byte
        if (A >= 3) | ((ptr as usize) % 8 == 0) {
            // SAFETY: by the above check, `ptr` is at least 8 byte aligned
            // SAFETY: the other safety requirements of `volatile_write8_zeroize_mem` are
            // also required by this function
            unsafe {
                internals::volatile_write8_zeroize(ptr, len);
            }
        } else {
            // SAFETY: the caller must uphold the contract of `volatile_write_zeroize_mem`
            unsafe {
                internals::volatile_write_zeroize(ptr, len);
            }
        }
        fence();
    }
}

/// This zeroizer uses inline asm with avx2 instructions if the pointer is 32
/// byte aligned, and otherwise uses `VolatileWrite8Zeroizer`. This zeroization
/// technique is available for x86_64 platforms with avx2 cpu support on stable,
/// and reasonably fast for 32 byte aligned pointers.
///
/// In addition to the volatile write we place a compiler fence right next to
/// the volatile write. This should not be necessary for secure zeroization
/// since the volatile semantics guarenties our writes are not elided, and they
/// can not be delayed since we are deallocating the memory after zeroization.
/// The use of this fence is therefore only a precaution.
///
/// This zeroization method can benefit (in terms of performance) from using the
/// [`MemZeroizer::zeroize_mem_aligned`] function instead of
/// [`MemZeroizer::zeroize_mem`] function if a minimum alignment is known
/// at compile time.
#[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
#[derive(Debug, Copy, Clone, Default)]
pub struct X86_64AvxZeroizer;

#[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
impl MemZeroizer for X86_64AvxZeroizer {
    unsafe fn zeroize_mem_aligned<const A: u8>(&self, mut ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // if we have 32 = 2^5 byte alignment then write 32 bytes at a time,
        // with 8 = 2^3 byte align do 8 bytes at a time, otherwise 1 byte at a time
        if (A >= 5) | ((ptr as usize) % 32 == 0) {
            // SAFETY: `ptr` is 32 byte aligned
            unsafe {
                ptr = internals::x86_64_simd32_unroll2_zeroize_align32_block32(ptr, len);
                // zeroize tail
                internals::volatile_write8_zeroize(ptr, len % 32);
            }
        } else if (A >= 3) | ((ptr as usize) % 8 == 0) {
            // SAFETY: `ptr` is 8 byte aligned
            unsafe {
                internals::volatile_write8_zeroize(ptr, len);
            }
        } else {
            // SAFETY: no alignment requirement
            unsafe {
                internals::volatile_write_zeroize(ptr, len);
            }
        }
        fence();
    }
}

/// This zeroizer uses inline asm with sse2 instructions if the pointer is 16
/// byte aligned, and otherwise uses `VolatileWrite8Zeroizer`. This zeroization
/// technique is available for x86_64 platforms with sse2 cpu support on stable,
/// and reasonably fast for 16 byte aligned pointers.
///
/// In addition to the volatile write we place a compiler fence right next to
/// the volatile write. This should not be necessary for secure zeroization
/// since the volatile semantics guarenties our writes are not elided, and they
/// can not be delayed since we are deallocating the memory after zeroization.
/// The use of this fence is therefore only a precaution.
///
/// This zeroization method can benefit (in terms of performance) from using the
/// [`MemZeroizer::zeroize_mem_aligned`] function instead of
/// [`MemZeroizer::zeroize_mem`] function if a minimum alignment is known
/// at compile time.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[derive(Debug, Copy, Clone, Default)]
pub struct X86_64Sse2Zeroizer;

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
impl MemZeroizer for X86_64Sse2Zeroizer {
    unsafe fn zeroize_mem_aligned<const A: u8>(&self, mut ptr: *mut u8, len: usize) {
        precondition_memory_range!(ptr, len);
        debug_precondition_logaligned!(A, ptr);
        // if we have 16 = 2^4 byte alignment then write 16 bytes at a time,
        // with 8 = 2^3 byte align do 8 bytes at a time, otherwise 1 byte at a time
        if (A >= 4) | ((ptr as usize) % 16 == 0) {
            // SAFETY: `ptr` is 16 byte aligned
            unsafe {
                ptr = internals::x86_64_simd16_unroll2_zeroize_align16_block16(ptr, len);
                // zeroize tail
                internals::volatile_write8_zeroize(ptr, len % 16);
            }
        } else if (A >= 3) | ((ptr as usize) % 8 == 0) {
            // SAFETY: `ptr` is 8 byte aligned
            unsafe {
                internals::volatile_write8_zeroize(ptr, len);
            }
        } else {
            // SAFETY: no alignment requirement
            unsafe {
                internals::volatile_write_zeroize(ptr, len);
            }
        }
        fence();
    }
}

/// Compiler fence.
///
/// Forces sequentially consistent access across this fence at compile time. At
/// runtime the CPU can still reorder memory accesses. This should not be
/// necessary for secure zeroization since the volatile semantics guaranties our
/// writes are not elided, and they can not be delayed since we are deallocating
/// the memory after zeroization. The use of this fence is therefore only a
/// precaution. For the same reasons it probably does not add security, it also
/// probably does not hurt performance significantly.
#[inline]
fn fence() {
    use core::sync::atomic::{compiler_fence, Ordering};

    compiler_fence(Ordering::SeqCst);
}

#[cfg(test)]
mod tests;
