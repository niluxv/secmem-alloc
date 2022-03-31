//! Utility functions for securely wiping memory, implemented using
//! cross-platform volatile writes.

use crate::macros::precondition_memory_range;
use mirai_annotations::debug_checked_precondition_eq;

/// Zeroize the memory pointed to by `ptr` and of size `len` bytes, by
/// overwriting it byte for byte using volatile writes.
///
/// This is guarantied to be not elided by the compiler.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
pub unsafe fn volatile_write_zeroize(mut ptr: *mut u8, len: usize) {
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
pub unsafe fn volatile_write8_zeroize(mut ptr: *mut u8, len: usize) {
    precondition_memory_range!(ptr, len);
    debug_checked_precondition_eq!((ptr as usize) % 8, 0);
    // first zeroize multiples of 8
    ptr = unsafe { zeroize_align8_block8(ptr, len) };
    // if `len` is not a multiple of 8 then the remainder (at most 7 bytes) needs to
    // be zeroized
    // SAFETY: `ptr` was incremented by a multiple of 8 by `zeroize_align8_block8`
    // still 8 byte aligned (so also 4)
    unsafe { zeroize_align4_tail8(ptr, len) };
}

/// Zeroize the memory pointed to by `ptr` for `len` rounded down to a multiple
/// of 8 bytes.
///
/// This function rounds down `len` to a multiple of 8 and then zeroizes the
/// memory pointed to by `ptr` for that length. This operation is guarantied to
/// be not elided by the compiler. If `len` is a multiple of 8 then this
/// zeroizes the entire specified block of memory. Returns a pointer to the byte
/// after the last zeroed byte, with the provenance of `ptr`.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
///
/// Furthermore, `ptr` *must* be at least 8 byte aligned.
pub unsafe fn zeroize_align8_block8(mut ptr: *mut u8, len: usize) -> *mut u8 {
    precondition_memory_range!(ptr, len);
    debug_checked_precondition_eq!((ptr as usize) % 8, 0);

    let nblocks = (len - len % 8) / 8;
    for _i in 0..nblocks {
        // SAFETY: `ptr` originally pointed into an allocation of `len` bytes so now,
        // after `_i` steps `len - 8*_i >= 8` bytes are left, so `ptr` is valid
        // for an 8 byte write SAFETY: `ptr` was originally 8 byte aligned by
        // caller contract and we only added a multiple of 8 so it is still 8
        // byte aligned
        unsafe {
            core::ptr::write_volatile(ptr.cast::<u64>(), 0u64);
        }
        // SAFETY: after increment, `ptr` points into the same allocation or (if `8*_i
        // == len`) at most one byte past it, so `add` is sound; `ptr` stays 8
        // byte aligned
        ptr = unsafe { ptr.add(8) };
    }
    ptr
}

/// Zeroize the memory pointed to by `ptr` and of size `len % 8` bytes.
///
/// This can be used to zeroize the bytes left unzeroized by
/// `zeroize_align8_block8` if `len` is not a multiple of 8. This operation is
/// guarantied to be not elided by the compiler.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
///
/// Furthermore, `ptr` *must* be at least 4 byte aligned.
pub unsafe fn zeroize_align4_tail8(mut ptr: *mut u8, len: usize) {
    precondition_memory_range!(ptr, len);
    debug_checked_precondition_eq!((ptr as usize) % 4, 0);

    if len % 8 >= 4 {
        // SAFETY: `ptr` is valid for `len % 8` bytes by caller contract
        // SAFETY: `ptr` is still 4 byte aligned by caller contract
        unsafe {
            core::ptr::write_volatile(ptr.cast::<u32>(), 0u32);
        }
        ptr = unsafe { ptr.add(4) };
    }
    // the final remainder (at most 3 bytes) is zeroed byte-for-byte
    // SAFETY: `ptr` has been incremented by a multiple of 4 <= `len` so `ptr`
    // points to an allocation of `len % 4` bytes, so `ptr` can be written to
    // and incremented `len % 4` times
    for _i in 0..(len % 4) {
        unsafe {
            core::ptr::write_volatile(ptr, 0u8);
        }
        ptr = unsafe { ptr.add(1) };
    }
}
