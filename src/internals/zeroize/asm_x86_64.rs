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
