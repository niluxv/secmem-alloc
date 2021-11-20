//! Small utilities used in other parts of the crate.
//!
//! Mainly stable replacements for nightly only functionality.

use core::ptr::NonNull;
use mirai_annotations::debug_checked_precondition;

#[cfg(not(feature = "nightly_core_intrinsics"))]
pub(crate) fn likely(b: bool) -> bool {
    b
}

#[cfg(not(feature = "nightly_core_intrinsics"))]
pub(crate) fn unlikely(b: bool) -> bool {
    b
}

#[cfg(feature = "nightly_core_intrinsics")]
pub(crate) fn likely(b: bool) -> bool {
    core::intrinsics::likely(b)
}

#[cfg(feature = "nightly_core_intrinsics")]
pub(crate) fn unlikely(b: bool) -> bool {
    core::intrinsics::unlikely(b)
}

/// Stable version of nightly only `NonNull::<[T]>::as_mut_ptr` from std.
pub(crate) fn nonnull_as_mut_ptr<T>(ptr: NonNull<[T]>) -> *mut T {
    ptr.as_ptr() as *mut T
}

/// Align pointer `ptr` upwards to `align`. The return value is a null-pointer
/// iff `ptr` cannot be aligned to `align`. The resulting pointer has the same
/// provenance as `ptr`.
///
/// # Safety
/// `align` must be a power of two (2). The resulting pointer is potentially
/// null and has the same provenance as `ptr` so be careful that it is in the
/// required memory range before dereferencing it.
pub(crate) unsafe fn align_ptr_mut(ptr: *mut u8, align: usize) -> *mut u8 {
    debug_checked_precondition!(align.is_power_of_two());
    // align `ptr` to `align` or `0` if not possible; as a usize
    // `align - 1` doesn't wrap as `align` is a power of 2, so >= 1
    let aligned: usize = (ptr as usize).wrapping_add(align - 1) & !(align - 1);
    // compute the difference with the original pointer
    let offset = aligned.wrapping_sub(ptr as usize);
    // add the offset to the original pointer; this way we keep the original pointer
    // provenance
    ptr.wrapping_add(offset)
}
