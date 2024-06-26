//! Helper functions for allocating memory and working with memory pages.

#[cfg(unix)]
use core::ffi::c_void;
use core::ptr::NonNull;
#[cfg(unix)]
use libc::{c_int, off_t, size_t};
#[cfg(feature = "std")]
use thiserror::Error;
#[cfg(windows)]
use winapi::ctypes::c_void;

/// Return the page size on the running system.
///
/// Is constant during the entire execution of a process.
// TODO: should we store the page size in a static to avoid repeat FFI calls to
// get the page size? with cross language LTO and static libc linking that
// shouldn't be necessary
pub fn page_size() -> usize {
    get_sys_page_size()
}

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        /// Page size shim for miri.
        #[cfg(not(tarpaulin_include))]
        fn get_sys_page_size() -> usize {
            4096
        }
    } else if #[cfg(unix)] {
        /// Return the page size on the running system by querying libc.
        fn get_sys_page_size() -> usize {
            unsafe {
                // the pagesize must always fit in a `size_t` (`usize`)
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    libc::sysconf(libc::_SC_PAGESIZE) as size_t
                }
            }
        }
    } else if #[cfg(windows)] {
        /// Return the page size on the running system by querying kernel32.lib.
        fn get_sys_page_size() -> usize {
            use winapi::um::sysinfoapi::{LPSYSTEM_INFO, GetSystemInfo, SYSTEM_INFO};

            let mut sysinfo = SYSTEM_INFO::default();
            let sysinfo_ptr: LPSYSTEM_INFO = &mut sysinfo as *mut SYSTEM_INFO;
            // SAFETY: `sysinfo_ptr` points to a valid (empty/all zeros) `SYSTEM_INFO`
            unsafe {
                GetSystemInfo(sysinfo_ptr)
            };
            // the pagesize must always fit in a `usize` (on windows it is a `u32`)
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                sysinfo.dwPageSize as usize
            }
        }
    }
}

/// Could not allocate a memory page.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(Error))]
#[cfg_attr(feature = "std", error("could not map a memory page"))]
pub struct PageAllocError;

/// Could not mlock a range of pages.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(Error))]
#[cfg_attr(
    feature = "std",
    error("could not lock the memory page to physical memory")
)]
struct MemLockError;

/// An single allocated page of memory.
pub struct Page {
    /// Pointer to the start of the page.
    page_ptr: NonNull<u8>,
    ///// This type owns a page of memory as raw bytes
    //_phantom_pagemem: PhantomData<[u8]>,
    /// Size of a memory page.
    ///
    /// It is not strictly necessary to store this as it is constant during the
    /// entire execution of a process. This will therefore at all times
    /// equal the result of `page_size`.
    // TODO: if we decide to store the page size in a static then this field can be removed
    page_size: usize,
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

    /// Get a mutable pointer to the start of the memory page.
    fn as_c_ptr_mut(&self) -> *mut c_void {
        self.as_ptr_mut() as *mut c_void
    }

    /// Get a non-mutable pointer to the start of the memory page.
    pub fn as_ptr(&self) -> *const u8 {
        self.page_ptr.as_ptr() as *const u8
    }
}

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        // miri shim
        #[cfg(not(tarpaulin_include))]
        impl Drop for Page {
            fn drop(&mut self) {
                let ptr = self.as_c_ptr_mut();
                let page_size = self.page_size();
                unsafe {
                    // SAFETY: we allocated/mapped this page in the constructor so it is safe to
                    // unmap now `munmap` also unlocks a page if it was locked so it is
                    // not necessary to `munlock` the page if it was locked.
                    //libc::munmap(ptr, self.page_size());
                    std::alloc::dealloc(
                        ptr as *mut u8,
                        std::alloc::Layout::from_size_align(page_size, page_size).unwrap(),
                    );
                }
                // SAFETY: `NonNull<u8>` and `usize` both do not drop so we need not
                // worry about subsequent drops
            }
        }
    } else if #[cfg(unix)] {
        impl Drop for Page {
            fn drop(&mut self) {
                let ptr = self.as_c_ptr_mut();
                unsafe {
                    // SAFETY: we allocated/mapped this page in the constructor so it is safe to
                    // unmap now `munmap` also unlocks a page if it was locked so it is
                    // not necessary to `munlock` the page if it was locked.
                    libc::munmap(ptr, self.page_size());
                }
                // SAFETY: `NonNull<u8>` and `usize` both do not drop so we need not
                // worry about subsequent drops
            }
        }
    } else if #[cfg(windows)] {
        impl Drop for Page {
            fn drop(&mut self) {
                use winapi::um::memoryapi::VirtualFree;
                use winapi::um::winnt::MEM_RELEASE;
                use winapi::shared::minwindef::LPVOID;

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
    }
}

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        // miri shims, better than nothing but not very accurate
        #[cfg(not(tarpaulin_include))]
        impl Page {
            fn alloc_new() -> Result<Self, PageAllocError> {
                let _addr: *mut c_void = core::ptr::null_mut();
                let page_size: size_t = page_size();
                let _prot: c_int = libc::PROT_READ | libc::PROT_WRITE;
                // NORESERVE disables backing the memory map with swap space
                let _flags = libc::MAP_PRIVATE | libc::MAP_NORESERVE | libc::MAP_ANONYMOUS;
                let _fd: c_int = -1;
                let _offset: off_t = 0;

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
                    })
                }
            }

            fn mlock(&mut self) -> Result<(), MemLockError> {
                let res = {
                    //libc::mlock(self.as_c_ptr_mut(), self.page_size())
                    let _ptr = self.as_c_ptr_mut();
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
    } else if #[cfg(unix)] {
        impl Page {
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
                cfg_if::cfg_if!{
                    if #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))] {
                        let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NOCORE;
                    } else {
                        let flags = libc::MAP_PRIVATE | libc::MAP_NORESERVE | libc::MAP_ANONYMOUS;
                    }
                }

                let fd: c_int = -1;
                let offset: off_t = 0;

                let page_ptr: *mut c_void = unsafe {
                    libc::mmap(addr, page_size, prot, flags, fd, offset)
                };

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
    } else if #[cfg(windows)] {
        impl Page {
            /// Allocate a new page of memory using `VirtualAlloc`.
            ///
            /// # Errors
            /// The function returns an `PageAllocError` if the `VirtualAlloc` call fails.
            fn alloc_new() -> Result<Self, PageAllocError> {
                use winapi::um::memoryapi::VirtualAlloc;
                use winapi::um::winnt::{MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE};
                use winapi::shared::{minwindef::{DWORD, LPVOID}, basetsd::SIZE_T};

                let addr: LPVOID = core::ptr::null_mut();
                let page_size: SIZE_T = page_size();
                let alloc_type: DWORD = MEM_RESERVE | MEM_COMMIT;
                let protect: DWORD = PAGE_READWRITE;

                let page_ptr: LPVOID = unsafe {
                    VirtualAlloc(addr, page_size, alloc_type, protect)
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
                    })
                }
            }

            /// Lock the memory page to physical memory.
            ///
            /// When this function returns successfully then the memory page is
            /// guarantied to be backed by physical memory, i.e. not (only) swapped.
            /// This guaranties the memory to not be swapped at all, except on hibernation
            /// or memory starvation. This is really the best we can achieve. If memory
            /// contents are really secret than there is no other solution than to
            /// use a swap space encrypted with an ephemeral secret key, and
            /// hibernation should be disabled (both on the OS level).
            fn lock(&mut self) -> Result<(), MemLockError> {
                use winapi::um::memoryapi::VirtualLock;
                use winapi::shared::minwindef::BOOL;

                let res: BOOL = unsafe { VirtualLock(self.as_c_ptr_mut(), self.page_size()) };

                if res == 0 {
                    Err(MemLockError)
                } else {
                    Ok(())
                }
            }

            /// Allocate a new page of memory using `VirtualAlloc` and `VirtualLock` page.
            ///
            /// The page is locked to force it into physical memory.
            ///
            /// # Errors
            /// The function returns an `PageAllocError` if the `VirtualAlloc` or `VirtualLock`
            /// call fails.
            pub fn alloc_new_lock() -> Result<Self, PageAllocError> {
                let mut page = Self::alloc_new()?;
                page.lock().map_err(|_| PageAllocError)?;
                Ok(page)
            }
        }
    }
}
