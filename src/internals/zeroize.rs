//! Utility functions for securely wiping memory.
//!
//! Contains wrappers around intrinsics and ffi functions necessary for the
//! [`crate::zeroize`] module.

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
    // SAFETY: the caller must uphold the safety contract
    unsafe {
        core::intrinsics::volatile_set_memory(ptr, val, len);
    }
}

/// Overwrite memory with zeros. This operation will not be elided by the
/// compiler.
///
/// This uses the `explicit_bzero` function present in many recent libcs.
///
/// # Safety
/// It's C. But the safety requirement is quite obvious: The caller *must*
/// ensure that `ptr` is valid for writes of `len` bytes, see the [`std::ptr`]
/// documentation. In particular this function is not atomic.
// In addition `ptr` needs to be properly aligned, but because we are talking
// about bytes (therefore byte alignment), it *always* is.
#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_env = "gnu",
    target_env = "musl"
))]
pub unsafe fn libc_explicit_bzero(ptr: *mut u8, len: usize) {
    // SAFETY: the caller must uphold the safety contract
    unsafe {
        libc::explicit_bzero(ptr as *mut libc::c_void, len as libc::size_t);
    }
}

/// Overwrite memory with zeros. This operation will not be elided by the
/// compiler.
///
/// This uses the `explicit_bzero` function present in many recent libcs.
///
/// # Safety
/// It's C. But the safety requirement is quite obvious: The caller *must*
/// ensure that `ptr` is valid for writes of `len` bytes, see the [`std::ptr`]
/// documentation. In particular this function is not atomic.
// In addition `ptr` needs to be properly aligned, but because we are talking
// about bytes (therefore byte alignment), it *always* is.
#[cfg(target_os = "netbsd")]
pub unsafe fn libc_explicit_bzero(ptr: *mut u8, len: usize) {
    // SAFETY: the caller must uphold the safety contract
    unsafe {
        libc::explicit_memset(
            ptr as *mut libc::c_void,
            0 as libc::c_int,
            len as libc::size_t,
        );
    }
}

/// Overwrite memory with zeros. This operation will not be elided by the
/// compiler.
///
/// This uses the `explicit_bzero` function present in many recent libcs.
///
/// # Safety
/// It's C. But the safety requirement is quite obvious: The caller *must*
/// ensure that `ptr` is valid for writes of `len` bytes, see the [`std::ptr`]
/// documentation. In particular this function is not atomic.
// In addition `ptr` needs to be properly aligned, but because we are talking
// about bytes (therefore byte alignment), it *always* is.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub unsafe fn libc_explicit_bzero(ptr: *mut u8, len: usize) {
    // SAFETY: the caller must uphold the safety contract
    unsafe {
        // the zero value is a `c_int` (`i32` by default), but then converted to
        // `unsigned char` (`u8`)
        libc::memset_s(
            ptr as *mut libc::c_void,
            len as libc::size_t,
            0 as libc::c_int,
            len as libc::size_t,
        );
    }
}

/// Overwrite memory with zeros. This operation will not be elided by the
/// compiler.
///
/// This uses inline assembly in C build and linked using `build.rs`. The
/// implementation makes use of the efficient `rep stosb` memory set
/// functionality on modern x86_64 cpus. This is especially fast for zeroizing
/// large amounts of data, works on stable, does not require a libc, but uses a
/// c compiler.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
// In addition `ptr` needs to be properly aligned, but because we are talking
// about bytes (therefore byte alignment), it *always* is.
#[cfg(all(target_arch = "x86_64", target_feature = "ermsb", feature = "cc"))]
pub unsafe fn c_asm_ermsb_zeroize(ptr: *mut u8, len: usize) {
    extern "C" {
        fn zeroize_volatile(ptr: *mut u8, count: usize);
    }
    // SAFETY: the caller must uphold the safety contract
    unsafe { zeroize_volatile(ptr, len) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "nightly_core_intrinsics")]
    #[cfg_attr(miri, ignore)] // TODO: remove
    #[test]
    fn test_volatile_memset() {
        let mut array: [u8; 128] = [0xAF; 128];
        unsafe {
            let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
            volatile_memset(ptr, 0, 128);
        }
        assert_eq!(array, [0u8; 128]);
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
        let mut array: [u8; 128] = [0xAF; 128];
        unsafe {
            let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
            libc_explicit_bzero(ptr, 128);
        }
        assert_eq!(array, [0u8; 128]);
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "ermsb", feature = "cc"))]
    #[test]
    #[cfg_attr(miri, ignore)] // ffi, asm
    fn test_asm_ermsb_zeroize() {
        let mut array: [u8; 128] = [0xAF; 128];
        unsafe {
            let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
            c_asm_ermsb_zeroize(ptr, 128);
        }
        assert_eq!(array, [0u8; 128]);
    }

    #[cfg(feature = "nightly_core_intrinsics")]
    #[cfg_attr(miri, ignore)] // TODO: remove
    #[test]
    fn test_volatile_memset_lowalign() {
        // ensure we get 8 byte aligned memory
        let mut array: [u64; 30] = [0x_AFAFAFAF_AFAFAFAF; 30];
        // zeroize everything but the first byte, so the pointer to the memory will have
        // an alignment of 1 byte
        unsafe {
            let array_ptr: *mut u64 = (&mut array[..]).as_mut_ptr();
            // 1 byte aligned
            let ptr: *mut u8 = ((array_ptr as usize) + 1) as *mut u8;
            // this should still be safe
            volatile_memset(ptr, 0, 30 * 8 - 1);
        }
        let mut expected: [u64; 30] = [0; 30];
        expected[0] = u64::from_ne_bytes([0x_AF, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&array[..], &expected[..]);
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
        // ensure we get 8 byte aligned memory
        let mut array: [u64; 30] = [0x_AFAFAFAF_AFAFAFAF; 30];
        // zeroize everything but the first byte, so the pointer to the memory will have
        // an alignment of 1 byte
        unsafe {
            let array_ptr: *mut u64 = (&mut array[..]).as_mut_ptr();
            // 1 byte aligned
            let ptr: *mut u8 = ((array_ptr as usize) + 1) as *mut u8;
            // this should still be safe
            libc_explicit_bzero(ptr, 30 * 8 - 1);
        }
        let mut expected: [u64; 30] = [0; 30];
        expected[0] = u64::from_ne_bytes([0x_AF, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&array[..], &expected[..]);
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "ermsb", feature = "cc"))]
    #[test]
    #[cfg_attr(miri, ignore)] // ffi, asm
    fn test_asm_ermsb_zeroize_lowalign() {
        // ensure we get 8 byte aligned memory
        let mut array: [u64; 30] = [0x_AFAFAFAF_AFAFAFAF; 30];
        // zeroize everything but the first byte, so the pointer to the memory will have
        // an alignment of 1 byte
        unsafe {
            let array_ptr: *mut u64 = (&mut array[..]).as_mut_ptr();
            // 1 byte aligned
            let ptr: *mut u8 = ((array_ptr as usize) + 1) as *mut u8;
            // this should still be safe
            c_asm_ermsb_zeroize(ptr, 30 * 8 - 1);
        }
        let mut expected: [u64; 30] = [0; 30];
        expected[0] = u64::from_ne_bytes([0x_AF, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&array[..], &expected[..]);
    }
}
