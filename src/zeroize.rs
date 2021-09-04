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
    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize);

    /// Zeroize the memory pointed to by `ptr` and of size `len` bytes, aligned
    /// at least `align`.
    ///
    /// This is guarantied to be not elided by the compiler.
    ///
    /// `ptr` must be at least `align` byte aligned, see the safety section
    /// below. The `align` value might be used to optimise out a branch on
    /// alignment if `align` is known at compile time.
    ///
    /// # Safety
    /// The caller *must* ensure that `ptr` is valid for writes of `len` bytes,
    /// see the [`std::ptr`] documentation. In particular this function is
    /// not atomic.
    ///
    /// Furthermore, `ptr` *must* be at least `align` byte aligned, and `align`
    /// must be a power of 2 (and therefore non-zero).
    ///
    /// # Performance
    /// The `align` value might be used to optimise out a branch on alignment if
    /// `align` is known at compile time. Using this method will at least
    /// not degrade performance relative to [`Self::zeroize_mem`] if `align` is
    /// known at compile time. Therefore it is fine to underestimate the
    /// alignment, especially if this underestimate can be known at compile
    /// time.
    unsafe fn zeroize_mem_minaligned(&self, ptr: *mut u8, len: usize, align: usize) {
        // SAFETY: caller must uphold the safety contract of `self.zeroize_mem` (and
        // more)
        unsafe { self.zeroize_mem(ptr, len) }
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
    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize) {
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
    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize) {
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
    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize) {
        // SAFETY: the caller must uphold the safety contract of
        // `internals::c_asm_ermsb_zeroize`
        unsafe {
            internals::c_asm_ermsb_zeroize(ptr, len);
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
    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize) {
        // SAFETY: the caller must uphold the safety contract of
        // `volatile_write_zeroize_mem`
        unsafe {
            volatile_write_zeroize_mem(ptr, len);
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
/// [`MemZeroizer::zeroize_mem_minaligned`] function instead of
/// [`MemZeroizer::zeroize_mem`] function if a minimum alignment might be known
/// at compile time.
#[derive(Debug, Copy, Clone, Default)]
pub struct VolatileWrite8Zeroizer;

impl MemZeroizer for VolatileWrite8Zeroizer {
    unsafe fn zeroize_mem_minaligned(&self, ptr: *mut u8, len: usize, align: usize) {
        debug_assert_ne!(align, 0);
        // if we have 8 byte alignment then write 8 bytes at a time, otherwise
        // byte-for-byte
        if align >= 8 {
            // SAFETY: by the above check, `ptr` is at least 8 byte aligned
            // SAFETY: the other safety requirements of `volatile_write8_zeroize_mem` are
            // also required by this function
            unsafe {
                volatile_write8_zeroize_mem(ptr, len);
            }
        } else {
            // SAFETY: the caller must uphold the contract of `volatile_write_zeroize_mem`
            unsafe {
                self.zeroize_mem(ptr, len);
            }
        }
        fence();
    }

    unsafe fn zeroize_mem(&self, ptr: *mut u8, len: usize) {
        if (ptr as usize) % 8 == 0 {
            // SAFETY: by the above check, `ptr` is at least 8 byte aligned
            // SAFETY: the other safety requirements of `volatile_write8_zeroize_mem` are
            // also required by this function
            unsafe {
                volatile_write8_zeroize_mem(ptr, len);
            }
        } else {
            // SAFETY: the caller must uphold the contract of `volatile_write_zeroize_mem`
            unsafe {
                volatile_write_zeroize_mem(ptr, len);
            }
        }
        fence();
    }
}

/// Zeroize the memory pointed to by `ptr` and of size `len` bytes, by
/// overwriting it byte for byte using volatile writes.
///
/// This is guarantied to be not elided by the compiler.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
unsafe fn volatile_write_zeroize_mem(ptr: *mut u8, len: usize) {
    for i in 0..len {
        // ptr as usize + i can't overlow because `ptr` is valid for writes of `len`
        let ptr_new: *mut u8 = ((ptr as usize) + i) as *mut u8;
        // SAFETY: `ptr` is valid for writes of `len` bytes, so `ptr_new` is valid for a
        // byte write SAFETY: byte writes only require byte alignment which
        // immediate
        unsafe {
            core::ptr::write_volatile(ptr_new, 0u8);
        }
    }
}

/// Zeroize the memory pointed to by `ptr` and of size `len` bytes, by
/// overwriting it 8 bytes at a time using volatile writes.
///
/// This is guarantied to be not elided by the compiler.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
///
/// Furthermore, `ptr` *must* be at least 8 byte aligned.
unsafe fn volatile_write8_zeroize_mem(ptr: *mut u8, len: usize) {
    debug_assert_eq!((ptr as usize) % 8, 0);
    let nblocks = (len - len % 8) / 8;
    for i in 0..nblocks {
        // ptr as usize + 8*i can't overlow because `ptr` is valid for writes of `len`
        // SAFETY: `8*i + 8 =<  len`, so `ptr_new` will be valid for 8 bytes write
        let ptr_new: *mut u8 = ((ptr as usize) + 8 * i) as *mut u8;
        // SAFETY: `ptr` is valid for writes of `len` bytes, so `ptr_new` is valid for 8
        // byte write SAFETY: `ptr` is 8 byte aligned, therefore `ptr_new` too
        // (a multiple of 8 is added)
        unsafe {
            core::ptr::write_volatile(ptr_new as *mut u64, 0u64);
        }
    }
    // if `len` is not a multiple of 8 then the remainder (at most 7 bytes) needs to
    // be zeroized if the remainder is at least 4 bytes we zero these with a
    // single write
    if len % 8 >= 4 {
        // `(ptr as usize) + (len - len % 8)` doesn't overflow since `ptr` is valid for
        // `len` byte writes and `len % 8` is non-zero SAFETY: `(len - len % 8)
        // + 4 =< len`
        let ptr_new: *mut u32 = ((ptr as usize) + (len - len % 8)) as *mut u32;
        // SAFETY: therefore, since `ptr` is valid for `len` byte writes, `ptr_new` is
        // valid for a 4 byte write SAFETY: `ptr` is 8 byte aligned, therefore
        // `ptr_new` too (a multiple of 8 is added)
        unsafe {
            core::ptr::write_volatile(ptr_new, 0u32);
        }
    }
    // the final remainder (at most 3 bytes) is zeroed byte-for-byte
    for i in 0..(len % 4) {
        // `(ptr as usize) - 1 + len - i` doesn't overflow overlow because `ptr` is
        // valid for `len` writes, therefore non-zero, and the `+ len` then can't
        // overflow since a write can't wrap the address space the trickery with
        // the minus results in the most performant machine code (saves 1 instruction
        // asm)
        let ptr_new: *mut u8 = ((ptr as usize) - 1 + len - i) as *mut u8;
        // SAFETY: `(ptr as usize) - 1 + len - i` ranges precisely `(len - len %
        // 4)..len` SAFETY: `ptr` is valid for writes of `len` bytes, so
        // `ptr_new` is valid for a byte write SAFETY: byte writes only require
        // byte alignment which immediate
        unsafe {
            core::ptr::write_volatile(ptr_new, 0u8);
        }
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
