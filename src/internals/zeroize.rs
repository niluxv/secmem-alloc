//! Utility functions for securely wiping memory.
//!
//! Utility functions for the [`crate::zeroize`] module, to securely wiping
//! memory, implemented using cross-platform pure Rust volatile writes.

use crate::macros::precondition_memory_range;
use crate::util::is_aligned_ptr_mut;
use mirai_annotations::debug_checked_precondition;

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
    debug_checked_precondition!(is_aligned_ptr_mut(ptr, 8));

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
    precondition_memory_range!(ptr, len % 8);
    debug_checked_precondition!(is_aligned_ptr_mut(ptr, 4));

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

#[cfg(test)]
mod tests {
    use super::*;
    use sptr::Strict;

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
        assert_eq!(new_ptr.addr(), (&array.0[256] as *const u8).addr())
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

    #[test]
    fn test_volatile_write_zeroize() {
        test_b128_zeroizer(|ptr, len| unsafe { volatile_write_zeroize(ptr, len) })
    }

    #[test]
    fn test_lowalign_volatile_write_zeroize() {
        test_b239_lowalign_zeroizer(|ptr, len| unsafe { volatile_write_zeroize(ptr, len) })
    }

    #[test]
    fn test_zeroize_align8_block8() {
        test_b257_align64_block_zeroizer(|ptr, len| unsafe { zeroize_align8_block8(ptr, len) })
    }
}
