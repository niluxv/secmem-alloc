//! Small utilities used in other parts of the crate.
//!
//! Mainly stable replacements for nightly only functionality.

use core::ptr::NonNull;
use mirai_annotations::debug_checked_precondition;
#[cfg(not(feature = "nightly_strict_provenance"))]
use sptr::Strict;

pub(crate) fn unlikely(b: bool) -> bool {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly_core_intrinsics")] {
            core::intrinsics::unlikely(b)
        } else {
            b
        }
    }
}

/// Stable version of nightly only `NonNull::<[T]>::as_mut_ptr` from std.
pub(crate) fn nonnull_as_mut_ptr<T>(ptr: NonNull<[T]>) -> *mut T {
    ptr.as_ptr() as *mut T
}

/// Round `x` up to the smallest multiple of `div` >= `x`, where `div` is
/// expected to be a power of 2.
///
/// If `div` is not a power of two (2), then the function might panic or give a
/// wrong  result.
pub(crate) fn align_up_usize(x: usize, div: usize) -> usize {
    debug_checked_precondition!(div.is_power_of_two());
    x.wrapping_add(div - 1) & !(div - 1)
}

/// Align pointer `ptr` upwards to `align`. The return value is a null-pointer
/// iff `ptr` cannot be aligned to `align`. The resulting pointer has the same
/// provenance as `ptr`.
///
/// # Safety
/// `align` must be a power of two (2). The resulting pointer is potentially
/// null and has the same provenance as `ptr` so be careful that it is in the
/// required memory range before dereferencing it.
pub(crate) unsafe fn align_up_ptr_mut(ptr: *mut u8, align: usize) -> *mut u8 {
    debug_checked_precondition!(align.is_power_of_two());
    // align `ptr` to `align` or make it null if not possible
    // `align - 1` doesn't wrap as `align` is a power of 2, so >= 1
    ptr.map_addr(|addr| align_up_usize(addr, align))
}

/// Returns `true` iff `ptr` is `align` byte aligned.
///
/// For the result to be correct, `align` must be a power of two (2).
/// Might panic if `align` is not a power of two.
pub(crate) fn is_aligned_ptr(ptr: *const u8, align: usize) -> bool {
    debug_checked_precondition!(align.is_power_of_two());
    ptr.addr() % align == 0
}

/// Returns the offset in bytes of `ptr` relative to `base`. Must not wrap.
///
/// # Safety
/// `ptr.addr()` must be at least as large as `base.addr()`.
// actually the implementation just panics or wraps when that is not the case
pub(crate) unsafe fn large_offset_from(ptr: *const u8, base: *const u8) -> usize {
    debug_checked_precondition!(ptr.addr() >= base.addr());
    ptr.addr() - base.addr()
}
