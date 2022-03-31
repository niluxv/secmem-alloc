//! Utility functions for securely wiping memory, implemented in asm for x86_64
//! cpus.

use crate::macros::precondition_memory_range;

/// Overwrite memory with zeros. This operation will not be elided by the
/// compiler.
///
/// This uses inline assembly in Rust. The implementation makes use of the
/// efficient `rep stosb` memory set functionality on modern x86_64 cpus. This
/// is very slow for small amounts of data but very efficient for zeroizing
/// large amounts of data (depending an CPU architecture though), works on
/// stable, and does not require a libc.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
// In addition `ptr` needs to be properly aligned, but because we are talking
// about bytes (therefore byte alignment), it *always* is.
#[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
pub unsafe fn asm_ermsb_zeroize(ptr: *mut u8, len: usize) {
    precondition_memory_range!(ptr, len);

    unsafe {
        core::arch::asm!(
            "rep stosb byte ptr es:[rdi], al",
            // `len` in the rcx register
            inout("rcx") len => _,
            // `ptr` int the rdi register
            inout("rdi") ptr => _,
            // zero byte to al (first byte of rax) register
            in("al") 0u8,
            options(nostack),
        );
    }
}

/// Zeroize the memory pointed to by `ptr` for `len` rounded down to a multiple
/// of 16 bytes.
///
/// This function rounds down `len` to a multiple of 16 and then zeroizes the
/// memory pointed to by `ptr` for that length. This operation is guarantied to
/// be not elided by the compiler. If `len` is a multiple of 16 then this
/// zeroizes the entire specified block of memory. Returns a pointer to the byte
/// after the last zeroed byte, with the provenance of `ptr`.
///
/// This uses sse2 instructions in inline asm to zeroize the memory with blocks
/// of 16 bytes at a time.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
///
/// Furthermore, `ptr` *must* be at least 16 byte aligned.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub unsafe fn x86_64_simd16_zeroize_align16_block16(mut ptr: *mut u8, len: usize) -> *mut u8 {
    use core::arch::x86_64 as arch;

    precondition_memory_range!(ptr, len);
    mirai_annotations::debug_checked_precondition_eq!((ptr as usize) % 16, 0);

    let nblocks = (len - len % 16) / 16;

    for _i in 0..nblocks {
        // SAFETY: `ptr` is valid for a `len >= nblocks*16` byte write, so we can write
        // `nblocks` times 16 bytes and increment `ptr` by 16 bytes; `ptr` stays 16 byte
        // aligned
        unsafe {
            // SAFETY: `ptr` originally pointed into an allocation of `len` bytes so now,
            // after `_i` steps `len - 16*_i >= 16` bytes are left, so `ptr` is valid
            // for a 16 byte write; also `ptr` is 16 byte aligned
            core::arch::asm!(
                "
                /* write 16 zero bytes to ptr */
                vmovdqa xmmword ptr [{0}], {1}
                ",
                in(reg) ptr,
                in(xmm_reg) arch::_mm_setzero_si128(),
                options(nostack),
            );
            // NOTE: increment `ptr` outside of the asm to maintain provenance
            // SAFETY: this stays within the memory where `ptr` is valid for writes and
            // maintains 16 byte alignment
            ptr = ptr.add(16);
        }
    }
    ptr
}

/// Zeroize the memory pointed to by `ptr` for `len` rounded down to a multiple
/// of 32 bytes.
///
/// This function rounds down `len` to a multiple of 32 and then zeroizes the
/// memory pointed to by `ptr` for that length. This operation is guarantied to
/// be not elided by the compiler. If `len` is a multiple of 32 then this
/// zeroizes the entire specified block of memory. Returns a pointer to the byte
/// after the last zeroed byte, with the provenance of `ptr`.
///
/// This uses avx2 instructions in inline asm to zeroize the memory with blocks
/// of 32 bytes at a time.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
///
/// Furthermore, `ptr` *must* be at least 32 byte aligned.
#[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
pub unsafe fn x86_64_simd32_zeroize_align32_block32(mut ptr: *mut u8, len: usize) -> *mut u8 {
    use core::arch::x86_64 as arch;

    precondition_memory_range!(ptr, len);
    mirai_annotations::debug_checked_precondition_eq!((ptr as usize) % 32, 0);

    let nblocks = (len - len % 32) / 32;

    for _i in 0..nblocks {
        // SAFETY: `ptr` is valid for a `len >= nblocks*32` byte write, so we can write
        // `nblocks` times 32 bytes and increment `ptr` by 32 bytes; `ptr` stays 32 byte
        // aligned
        unsafe {
            // SAFETY: `ptr` originally pointed into an allocation of `len` bytes so now,
            // after `_i` steps `len - 32*_i >= 32` bytes are left, so `ptr` is valid
            // for a 32 byte write; also `ptr` is 32 byte aligned
            core::arch::asm!(
                "
                /* write 32 zero bytes to ptr */
                vmovdqa ymmword ptr [{0}], {1}
                ",
                in(reg) ptr,
                in(ymm_reg) arch::_mm256_setzero_si256(),
                options(nostack),
            );
            // NOTE: increment `ptr` outside of the asm to maintain provenance
            // SAFETY: this stays within the memory where `ptr` is valid for writes and
            // maintains 32 byte alignment
            ptr = ptr.add(32);
        }
    }
    ptr
}

/// Zeroize the memory pointed to by `ptr` for `len` rounded down to a multiple
/// of 64 bytes.
///
/// This function rounds down `len` to a multiple of 64 and then zeroizes the
/// memory pointed to by `ptr` for that length. This operation is guarantied to
/// be not elided by the compiler. If `len` is a multiple of 64 then this
/// zeroizes the entire specified block of memory. Returns a pointer to the byte
/// after the last zeroed byte, with the provenance of `ptr`.
///
/// This uses avx512 instructions in inline asm to zeroize the memory with
/// blocks of 64 bytes at a time.
///
/// # Safety
/// The caller *must* ensure that `ptr` is valid for writes of `len` bytes, see
/// the [`std::ptr`] documentation. In particular this function is not atomic.
///
/// Furthermore, `ptr` *must* be at least 64 byte aligned.
#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f",
    feature = "nightly_stdsimd"
))]
pub unsafe fn x86_64_simd64_zeroize_align64_block64(mut ptr: *mut u8, len: usize) -> *mut u8 {
    use core::arch::x86_64 as arch;

    precondition_memory_range!(ptr, len);
    mirai_annotations::debug_checked_precondition_eq!((ptr as usize) % 64, 0);

    let nblocks = (len - len % 64) / 64;

    for _i in 0..nblocks {
        // SAFETY: `ptr` is valid for a `len >= nblocks*64` byte write, so we can write
        // `nblocks` times 64 bytes and increment `ptr` by 64 bytes; `ptr` stays 64 byte
        // aligned
        unsafe {
            // SAFETY: `ptr` originally pointed into an allocation of `len` bytes so now,
            // after `_i` steps `len - 64*_i >= 64` bytes are left, so `ptr` is valid
            // for a 64 byte write; also `ptr` is 64 byte aligned
            core::arch::asm!(
                "
                /* write 64 zero bytes to ptr */
                vmovdqa64 zmmword ptr [{0}], {1}
                ",
                in(reg) ptr,
                in(zmm_reg) arch::_mm512_setzero_si512(),
                options(nostack),
            );
            // NOTE: increment `ptr` outside of the asm to maintain provenance
            // SAFETY: this stays within the memory where `ptr` is valid for writes and
            // maintains 64 byte alignment
            ptr = ptr.add(64);
        }
    }
    ptr
}
