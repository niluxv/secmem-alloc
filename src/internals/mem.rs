//! Helper functions for allocating memory and working with memory pages.

use core::ptr::NonNull;

/// An single allocated page of memory.
pub struct Page {
    /// Pointer to the start of the page.
    page_ptr: NonNull<u8>,
    /// Size of a memory page.
    ///
    /// It is not strictly necessary to store this as it is constant during the
    /// entire execution of a process. This will therefore at all times
    /// equal the result of `page_size`.
    page_size: usize,
    /// This type owns a page of memory as raw bytes
    _phantom_pagemem: core::marker::PhantomData<[u8]>,
}

impl Page {
    /// Get [`NonNull`] pointer to the page.
    pub fn page_ptr_nonnull(&self) -> NonNull<u8> {
        self.page_ptr
    }

    /// Get the page size of the memory page.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Get a mutable pointer to the start of the memory page.
    pub fn as_ptr_mut(&self) -> *mut u8 {
        self.page_ptr.as_ptr()
    }

    /// Get a non-mutable pointer to the start of the memory page.
    pub fn as_ptr(&self) -> *const u8 {
        self.page_ptr.as_ptr() as *const u8
    }
}

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        mod miri;
        pub use miri::PageAllocError;
    } else if #[cfg(unix)] {
        mod unix;
        pub use unix::PageAllocError;
    } else if #[cfg(windows)] {
        mod windows;
        pub use windows::PageAllocError;
    }
}
