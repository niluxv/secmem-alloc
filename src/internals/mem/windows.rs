//! Windows `VirtualAlloc` memory page allocation.

use super::{MemLockError, Page, PageAllocError};

use core::ptr::NonNull;
use winapi::ctypes::c_void;

/// Return the page size on the running system by querying kernel32.lib.
pub fn page_size() -> usize {
    use winapi::um::sysinfoapi::{GetSystemInfo, LPSYSTEM_INFO, SYSTEM_INFO};

    let mut sysinfo = SYSTEM_INFO::default();
    let sysinfo_ptr: LPSYSTEM_INFO = &mut sysinfo as *mut SYSTEM_INFO;
    // SAFETY: `sysinfo_ptr` points to a valid (empty/all zeros) `SYSTEM_INFO`
    unsafe { GetSystemInfo(sysinfo_ptr) };
    // the pagesize must always fit in a `usize` (on windows it is a `u32`)
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        sysinfo.dwPageSize as usize
    }
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
    fn alloc_new() -> Result<Self, PageAllocError> {
        use winapi::shared::basetsd::SIZE_T;
        use winapi::shared::minwindef::{DWORD, LPVOID};
        use winapi::um::memoryapi::VirtualAlloc;
        use winapi::um::winnt::{MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE};

        let addr: LPVOID = core::ptr::null_mut();
        let page_size: SIZE_T = page_size();
        let alloc_type: DWORD = MEM_RESERVE | MEM_COMMIT;
        let protect: DWORD = PAGE_READWRITE;

        let page_ptr: LPVOID = unsafe { VirtualAlloc(addr, page_size, alloc_type, protect) };

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

    /// Lock the memory page to physical memory.
    ///
    /// When this function returns successfully then the memory page is
    /// guarantied to be backed by physical memory, i.e. not (only) swapped.
    /// This guaranties the memory to not be swapped at all, except on
    /// hibernation or memory starvation. This is really the best we can
    /// achieve. If memory contents are really secret than there is no other
    /// solution than to use a swap space encrypted with an ephemeral secret
    /// key, and hibernation should be disabled (both on the OS level).
    fn lock(&mut self) -> Result<(), MemLockError> {
        use winapi::shared::minwindef::BOOL;
        use winapi::um::memoryapi::VirtualLock;

        let res: BOOL = unsafe { VirtualLock(self.as_c_ptr_mut(), self.page_size()) };

        if res == 0 {
            Err(MemLockError)
        } else {
            Ok(())
        }
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
        let mut page = Self::alloc_new()?;
        page.lock().map_err(|_| PageAllocError)?;
        Ok(page)
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        use winapi::shared::minwindef::LPVOID;
        use winapi::um::memoryapi::VirtualFree;
        use winapi::um::winnt::MEM_RELEASE;

        let ptr: LPVOID = self.as_c_ptr_mut();
        unsafe {
            // SAFETY: we allocated/mapped this page in the constructor so it is safe to
            // unmap now
            VirtualFree(ptr, 0, MEM_RELEASE);
        }
        // SAFETY: `NonNull<u8>` and `usize` both do not drop so we need not
        // worry about subsequent drops
    }
}
