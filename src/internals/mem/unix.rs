//! Unix `mmap` private anonymous memory pages.

use super::{MemLockError, Page, PageAllocError};

use core::ffi::c_void;
use core::ptr::NonNull;
use libc::{c_int, off_t, size_t};

/// Return the page size on the running system by querying libc.
pub fn page_size() -> usize {
    unsafe {
        // the pagesize must always fit in a `size_t` (`usize`)
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            libc::sysconf(libc::_SC_PAGESIZE) as size_t
        }
    }
}

impl Page {
    /// Get a mutable pointer to the start of the memory page.
    fn as_c_ptr_mut(&self) -> *mut c_void {
        self.as_ptr_mut() as *mut c_void
    }

    /// Allocate a new page of memory using (anonymous) `mmap` with the
    /// noreserve flag.
    ///
    /// The noreserve flag disables swapping of the memory page. As a
    /// consequence, the OS may unmap the page of memory, in which case
    /// writing to it causes a SIGSEGV. Therefore, the page
    /// should be mlocked before actual use.
    ///
    /// # Errors
    /// The function returns an `PageAllocError` if the `mmap` call fails.
    fn alloc_new_noreserve() -> Result<Self, PageAllocError> {
        let addr: *mut c_void = core::ptr::null_mut();
        let page_size: size_t = page_size();
        let prot: c_int = libc::PROT_READ | libc::PROT_WRITE;
        // NORESERVE disables backing the memory map with swap space
        // it is not available (anymore) on FreeBSD/DragonFlyBSD (never implemented)
        // also unimplemented on other BSDs, but the flag is there for compat...
        // FreeBSD + DragonFlyBSD have a `MAP_NOCORE` flag which excludes this memory
        // from being included in a core dump (but ideally, disable core dumps entirely)
        cfg_if::cfg_if! {
            if #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))] {
                let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NOCORE;
            } else {
                let flags = libc::MAP_PRIVATE | libc::MAP_NORESERVE | libc::MAP_ANONYMOUS;
            }
        }

        let fd: c_int = -1;
        let offset: off_t = 0;

        let page_ptr: *mut c_void = unsafe { libc::mmap(addr, page_size, prot, flags, fd, offset) };

        if page_ptr.is_null() || page_ptr == libc::MAP_FAILED {
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

    /// Lock the memory page to physical memory.
    ///
    /// When this function returns successfully then the memory page is
    /// guarantied to be backed by physical memory, i.e. not (only) swapped.
    /// In combination with the noreserve flag during the allocation, this
    /// guaranties the memory to not be swapped at all, except on hibernation
    /// or memory starvation. This is really the best we can achieve. If memory
    /// contents are really secret than there is no other solution than to
    /// use a swap space encrypted with an ephemeral secret key, and
    /// hibernation should be disabled (both on the OS level).
    fn mlock(&mut self) -> Result<(), MemLockError> {
        let res = unsafe { libc::mlock(self.as_c_ptr_mut(), self.page_size()) };

        if res == 0 {
            Ok(())
        } else {
            Err(MemLockError)
        }
    }

    /// Allocate a new page of memory using (anonymous) `mmap` with the
    /// noreserve flag and mlock page.
    ///
    /// The noreserve flag disables swapping of the memory page. The page is
    /// then mlocked to force it into physical memory.
    ///
    /// # Errors
    /// The function returns an `PageAllocError` if the `mmap` or `mlock` call
    /// fails.
    pub fn alloc_new_lock() -> Result<Self, PageAllocError> {
        let mut page = Self::alloc_new_noreserve()?;
        page.mlock().map_err(|_| PageAllocError)?;
        Ok(page)
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let ptr = self.as_c_ptr_mut();
        unsafe {
            // SAFETY: we allocated/mapped this page in the constructor so it is safe to
            // unmap now. `munmap` also unlocks a page if it was locked so it is
            // not necessary to `munlock` the page if it was locked.
            libc::munmap(ptr, self.page_size());
        }
        // SAFETY: `NonNull<u8>` and `usize` both do not drop so we need not
        // worry about subsequent drops
    }
}
