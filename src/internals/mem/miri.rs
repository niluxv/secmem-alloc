//! Miri shims for memory management. Not accurate, but better than nothing.

use super::{MemLockError, Page, PageAllocError};
use core::ptr::NonNull;

/// Page size shim for miri.
#[cfg(not(tarpaulin_include))]
pub fn page_size() -> usize {
    4096
}

#[cfg(not(tarpaulin_include))]
impl Page {
    fn alloc_new() -> Result<Self, PageAllocError> {
        let page_size = page_size();

        let page_ptr: *mut u8 = unsafe {
            //libc::mmap(_addr, page_size, _prot, _flags, _fd, _offset)
            std::alloc::alloc_zeroed(
                std::alloc::Layout::from_size_align(page_size, page_size).unwrap(),
            )
        };

        if page_ptr.is_null() {
            Err(PageAllocError)
        } else {
            let page_ptr = unsafe {
                // SAFETY: we just checked that `page_ptr` is non-null
                NonNull::new_unchecked(page_ptr as *mut u8)
            };
            Ok(Self {
                page_ptr,
                page_size,
                _phantom_pagemem: core::marker::PhantomData,
            })
        }
    }

    fn mlock(&mut self) -> Result<(), MemLockError> {
        let res = {
            //libc::mlock(self.as_c_ptr_mut(), self.page_size())
            let _ptr = self.as_ptr_mut();
            let _ps = self.page_size();
            0
        };

        if res == 0 {
            Ok(())
        } else {
            Err(MemLockError)
        }
    }

    pub fn alloc_new_lock() -> Result<Self, PageAllocError> {
        let mut page = Self::alloc_new()?;
        // if this fails then `page` is deallocated by it's drop implementation
        page.mlock().map_err(|_| PageAllocError)?;
        Ok(page)
    }
}

#[cfg(not(tarpaulin_include))]
impl Drop for Page {
    fn drop(&mut self) {
        let ptr = self.as_ptr_mut();
        let page_size = self.page_size();
        // SAFETY: we allocated this page in the constructor so it is safe to deallocate
        // now.
        unsafe {
            //libc::munmap(ptr, self.page_size());
            std::alloc::dealloc(
                ptr,
                std::alloc::Layout::from_size_align(page_size, page_size).unwrap(),
            );
        }
    }
}
