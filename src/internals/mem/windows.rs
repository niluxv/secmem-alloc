//! Windows `VirtualAlloc` memory page allocation.

use super::Page;

use core::ffi::c_void;
use core::ptr::NonNull;

/// Return the page size on the running system by querying kernel32.lib.
pub fn page_size() -> usize {
    use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

    let mut sysinfo = SYSTEM_INFO::default();
    let sysinfo_ptr = &mut sysinfo as *mut SYSTEM_INFO;
    // SAFETY: `sysinfo_ptr` points to a valid (empty/all zeros) `SYSTEM_INFO`
    unsafe { GetSystemInfo(sysinfo_ptr) };
    // the pagesize must always fit in a `usize` (on windows it is a `u32`)
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        sysinfo.dwPageSize as usize
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum PageAllocError {
    #[cfg_attr(feature = "std", error("could not map a memory page"))]
    VirtualAlloc,
    #[cfg_attr(feature = "std", error("could not lock memory page: {0}"))]
    VirtualLock(windows::core::Error),
}

impl Page {
    /// Get a mutable pointer to the start of the memory page.
    fn as_c_ptr_mut(&self) -> *mut c_void {
        self.as_ptr_mut() as *mut c_void
    }

    /// Allocate a new page of memory using `VirtualAlloc`.
    ///
    /// # Errors
    /// The function returns an `PageAllocError` if the `VirtualAlloc` call
    /// fails.
    fn alloc_new() -> Result<Self, ()> {
        use windows::Win32::System::Memory::{
            VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
            VIRTUAL_ALLOCATION_TYPE,
        };

        let page_size = page_size();
        let alloc_type: VIRTUAL_ALLOCATION_TYPE = MEM_RESERVE | MEM_COMMIT;
        let protect: PAGE_PROTECTION_FLAGS = PAGE_READWRITE;

        let page_ptr: *mut c_void = unsafe { VirtualAlloc(None, page_size, alloc_type, protect) };

        if page_ptr.is_null() {
            Err(())
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
    /// This guaranties the memory to not be swapped at all, except on
    /// hibernation or memory starvation. This is really the best we can
    /// achieve. If memory contents are really secret than there is no other
    /// solution than to use a swap space encrypted with an ephemeral secret
    /// key, and hibernation should be disabled (both on the OS level).
    fn lock(&mut self) -> Result<(), windows::core::Error> {
        use windows::Win32::System::Memory::VirtualLock;

        unsafe { VirtualLock(self.as_c_ptr_mut(), self.page_size()) }
    }

    /// Allocate a new page of memory using `VirtualAlloc` and `VirtualLock`
    /// page.
    ///
    /// The page is locked to force it into physical memory.
    ///
    /// # Errors
    /// The function returns an `PageAllocError` if the `VirtualAlloc` or
    /// `VirtualLock` call fails.
    pub fn alloc_new_lock() -> Result<Self, PageAllocError> {
        let mut page = Self::alloc_new().map_err(|_| PageAllocError::VirtualAlloc)?;
        page.lock().map_err(|e| PageAllocError::VirtualLock(e))?;
        Ok(page)
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        use windows::Win32::System::Memory::{VirtualFree, MEM_RELEASE};

        // SAFETY: we allocated/mapped this page in the constructor so it is safe to
        // unmap now
        unsafe { VirtualFree(self.as_c_ptr_mut(), 0, MEM_RELEASE) }.unwrap();
        // SAFETY: `NonNull<u8>` and `usize` both do not drop so we need not
        // worry about subsequent drops
    }
}
