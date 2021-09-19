//! Small utilities used in other parts of the crate.
//!
//! Mainly stable replacements for nightly only functionality.

use core::ptr::NonNull;

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
