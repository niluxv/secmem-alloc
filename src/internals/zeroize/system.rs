//! Bindings to system functions for securely wiping memory.

use crate::macros::precondition_memory_range;

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
    all(target_env = "gnu", unix),
    target_env = "musl"
))]
pub unsafe fn libc_explicit_bzero(ptr: *mut u8, len: usize) {
    precondition_memory_range!(ptr, len);
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
#[cfg(all(target_env = "gnu", windows))]
pub unsafe fn libc_explicit_bzero(ptr: *mut u8, len: usize) {
    precondition_memory_range!(ptr, len);
    extern "C" {
        fn explicit_bzero(ptr: *mut libc::c_void, len: libc::size_t);
    }

    // SAFETY: the caller must uphold the safety contract
    unsafe {
        explicit_bzero(ptr as *mut libc::c_void, len as libc::size_t);
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
    precondition_memory_range!(ptr, len);
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
    precondition_memory_range!(ptr, len);
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
