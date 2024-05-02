//! Miri shims for memory management. Not accurate, but better than nothing.

use super::Page;
use core::ptr::NonNull;

/// Page size shim for miri.
#[cfg(not(tarpaulin_include))]
pub fn page_size() -> usize {
    4096
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum PageAllocError {
    #[cfg_attr(feature = "std", error("trying to create invalid layout"))]
    Layout(std::alloc::LayoutError),
    #[cfg_attr(feature = "std", error("could not allocate memory"))]
    Alloc,
    #[cfg_attr(feature = "std", error("could not lock memory"))]
    Lock,
}

#[cfg(not(tarpaulin_include))]
impl Page {
    fn alloc_new() -> Result<Self, PageAllocError> {
        let page_size = page_size();

        //libc::mmap(_addr, page_size, _prot, _flags, _fd, _offset)
        let layout = std::alloc::Layout::from_size_align(page_size, page_size)
            .map_err(|e| PageAllocError::Layout(e))?;
        let page_ptr: *mut u8 = unsafe { std::alloc::alloc_zeroed(layout) };

        if page_ptr.is_null() {
            Err(PageAllocError::Alloc)
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

    fn mlock(&mut self) -> Result<(), PageAllocError> {
        let res = {
            //libc::mlock(self.as_c_ptr_mut(), self.page_size())
            let _ptr = self.as_ptr_mut();
            let _ps = self.page_size();
            0
        };

        if res == 0 {
            Ok(())
        } else {
            Err(PageAllocError::Lock)
        }
    }

    pub fn alloc_new_lock() -> Result<Self, PageAllocError> {
        let mut page = Self::alloc_new()?;
        // if this fails then `page` is deallocated by it's drop implementation
        page.mlock()?;
        Ok(page)
    }
}

#[cfg(not(tarpaulin_include))]
impl Drop for Page {
    fn drop(&mut self) {
        let ptr = self.as_ptr_mut();
        let page_size = self.page_size();

        //libc::munmap(ptr, self.page_size());
        let layout = std::alloc::Layout::from_size_align(page_size, page_size).unwrap();
        // SAFETY: we allocated this page in the constructor so it is safe to deallocate
        // now.
        unsafe { std::alloc::dealloc(ptr, layout) };
    }
}
