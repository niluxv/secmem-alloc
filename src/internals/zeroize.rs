//! Utility functions for securely wiping memory.
//!
//! Contains wrappers around intrinsics and ffi functions necessary for the
//! [`crate::zeroize`] module.

#[cfg(target_arch = "x86_64")]
mod asm_x86_64;
#[cfg(target_arch = "x86_64")]
pub use asm_x86_64::*;

mod system;
pub use system::*;

mod volatile_write;
pub use volatile_write::*;

/// Volatile write byte to memory.
///
/// This uses the [`core::intrinsics::volatile_set_memory`] intrinsic and can
/// only be used on nightly, with the `nightly` feature enabled.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
// In addition `ptr` needs to be properly aligned, but because we are talking
// about bytes (therefore byte alignment), it *always* is.
#[cfg(feature = "nightly_core_intrinsics")]
pub unsafe fn volatile_memset(ptr: *mut u8, val: u8, len: usize) {
    crate::macros::precondition_memory_range!(ptr, len);
    // SAFETY: the caller must uphold the safety contract
    unsafe {
        core::intrinsics::volatile_set_memory(ptr, val, len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_b128_zeroizer<Z: FnOnce(*mut u8, usize)>(zeroize: Z) {
        let mut array: [u8; 128] = [0xAF; 128];
        let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
        zeroize(ptr, 128);
        assert_eq!(array, [0u8; 128]);
    }

    #[repr(align(64))]
    struct B255Align64([u8; 257]);

    fn test_b257_align64_block_zeroizer<Z: FnOnce(*mut u8, usize) -> *mut u8>(zeroize: Z) {
        let mut array: B255Align64 = B255Align64([0xAF; 257]);
        let ptr: *mut u8 = (&mut array.0[..]).as_mut_ptr();
        let new_ptr = zeroize(ptr, 257);

        assert_eq!(array.0[0..256], [0u8; 256]);
        assert_eq!(array.0[256], 0xAF);
        assert_eq!(new_ptr as usize, &array.0[256] as *const u8 as usize)
    }

    fn test_b239_lowalign_zeroizer<Z: FnOnce(*mut u8, usize)>(zeroize: Z) {
        // ensure we get 8 byte aligned memory
        let mut array: [u64; 30] = [0x_AFAFAFAF_AFAFAFAF; 30];

        // zeroize everything but the first byte, so the pointer to the memory will have
        // an alignment of 1 byte

        let array_ptr: *mut u64 = (&mut array[..]).as_mut_ptr();
        // 1 byte aligned; SAFETY: resulting `ptr` still pointing in array
        let ptr: *mut u8 = unsafe { array_ptr.cast::<u8>().add(1) };
        // this should still be safe
        zeroize(ptr, 30 * 8 - 1);

        let mut expected: [u64; 30] = [0; 30];
        expected[0] = u64::from_ne_bytes([0x_AF, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&array[..], &expected[..]);
    }

    #[cfg(feature = "nightly_core_intrinsics")]
    #[test]
    fn test_volatile_memset() {
        test_b128_zeroizer(|ptr: *mut u8, len: usize| unsafe { volatile_memset(ptr, 0, len) })
    }

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
    #[test]
    #[cfg_attr(miri, ignore)] // ffi
    fn test_explicit_bzero() {
        test_b128_zeroizer(|ptr: *mut u8, len: usize| unsafe { libc_explicit_bzero(ptr, len) })
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
    #[test]
    #[cfg_attr(miri, ignore)] // asm
    fn test_asm_ermsb_zeroize() {
        test_b128_zeroizer(|ptr: *mut u8, len: usize| unsafe { asm_ermsb_zeroize(ptr, len) })
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
    #[test]
    fn test_volatile_write_zeroize() {
        test_b128_zeroizer(|ptr: *mut u8, len: usize| unsafe { volatile_write_zeroize(ptr, len) })
    }

    #[cfg(feature = "nightly_core_intrinsics")]
    #[test]
    fn test_volatile_memset_lowalign() {
        test_b239_lowalign_zeroizer(|ptr: *mut u8, len: usize| unsafe {
            volatile_memset(ptr, 0, len)
        })
    }

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
    #[test]
    #[cfg_attr(miri, ignore)] // ffi
    fn test_explicit_bzero_lowalign() {
        test_b239_lowalign_zeroizer(|ptr: *mut u8, len: usize| unsafe {
            libc_explicit_bzero(ptr, len)
        })
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
    #[test]
    #[cfg_attr(miri, ignore)] // asm
    fn test_asm_ermsb_zeroize_lowalign() {
        test_b239_lowalign_zeroizer(|ptr: *mut u8, len: usize| unsafe {
            asm_ermsb_zeroize(ptr, len)
        })
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
    #[test]
    fn test_volatile_write_zeroize_lowalign() {
        test_b239_lowalign_zeroizer(|ptr: *mut u8, len: usize| unsafe {
            volatile_write_zeroize(ptr, len)
        })
    }

    #[test]
    fn test_zeroize_align8_block8() {
        test_b257_align64_block_zeroizer(|ptr, len| unsafe { zeroize_align8_block8(ptr, len) })
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    #[cfg_attr(miri, ignore)] // asm
    fn test_x86_64_simd16_zeroize_align16_block16() {
        test_b257_align64_block_zeroizer(|ptr, len| unsafe {
            x86_64_simd16_zeroize_align16_block16(ptr, len)
        })
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    #[test]
    #[cfg_attr(miri, ignore)] // asm
    fn test_x86_64_simd32_zeroize_align32_block32() {
        test_b257_align64_block_zeroizer(|ptr, len| unsafe {
            x86_64_simd32_zeroize_align32_block32(ptr, len)
        })
    }

    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        feature = "nightly_stdsimd"
    ))]
    #[test]
    #[cfg_attr(miri, ignore)] // asm
    fn test_x86_64_simd64_zeroize_align64_block64() {
        test_b257_align64_block_zeroizer(|ptr, len| unsafe {
            x86_64_simd64_zeroize_align64_block64(ptr, len)
        })
    }
}
