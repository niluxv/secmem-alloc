//! An allocator designed to handle security sensitive allocations, i.e. heap
//! memory with confidential contents.
//!
//! This can be used to store e.g. passwords and secret cryptographic keys in
//! memory. It is not designed to be performant or light on system resources.
//!
//! The allocator tries to never get swapped out using `mlock` on linux. The
//! amount of memory that can be `mlock`ed is very limited for unprivileged
//! processes so use with care. Allocating too much memory using this allocator
//! (exceeding the `mlock` limit) causes the program to OOM abort using
//! [`alloc::alloc::handle_alloc_error`]. A process with `CAP_SYS_RESOURCE` can
//! change the `mlock` limit using `setrlimit` from libc (available in rust
//! through the `secmem-proc` crate).
//!
//! Various security measures are implemented:
//! - Zeroization of memory on drop.
//! - Non-swappable locked memory.
//! - Memory is not in the program break or global allocator memory pool,
//!   therefore at a less predictable address (even when the address to memory
//!   in the global allocator leaks). This *could* make some exploits harder,
//!   but not impossible.

use crate::allocator_api::{AllocError, Allocator};
use crate::internals::mem;
use crate::util::{nonnull_as_mut_ptr, unlikely};
use crate::zeroize::{DefaultMemZeroizer, MemZeroizer};
use core::alloc::Layout;
use core::cell::Cell;
use core::ptr::{self, NonNull};

/// Memory allocator for confidential memory. See the module level
/// documentation.
///
/// Memory allocator which is backed by a single page of memory. Allocation
/// works like in a bump allocator. This is very efficient for stacked
/// allocations, i.e. a latter allocation drops before an earlier allocation. If
/// allocations are deallocated in a different order, then memory can not be
/// reused until everything is deallocated.
///
/// Since the allocator is backed by a single page, only 4 KiB of memory (on
/// Linux with default configuration) can be allocated with a single. Exceeding
/// this limit causes the allocator to error on allocation requests!
///
/// This is not a zero sized type and should not be dropped before all it's
/// memory is deallocated. The same allocator instance must be used for
/// allocation and deallocation.
///
/// # Panics
/// If debug assertions are enabled, *some* of the safety requirement for using
/// the allocator are checked. In addition, memory leaks are then checked (at
/// drop). Therefore, memory allocated with this allocated should not leak!
///
/// # Errors
/// Allocation functions return errors when the requested allocation does not
/// fit what is left of the backing page of memory. In addition, zero sized
/// allocations are not allowed (but cause only an allocation error, no UB like
/// with `GlobalAlloc`).
///
/// # Memory fragmentation
/// This allocator is basically a bump allocator, and hence suffers from memory
/// fragmentation: memory can only be reused once all allocations are
/// deallocated, or if the allocator is used in a strictly (first-in last-out)
/// stack like manner with at most 8 byte aligned allocations. When
/// the allocator is used for a bunch of allocations which need to live for
/// approximately the same lifetime memory fragmentation is not an issue.
/// Otherwise, it might be a good idea to use the allocation in a filo stack
/// like manner, that is, always only deallocate, shrink or grow the
/// last created allocation, and request at most 8 byte alignment for all but
/// the first allocation.
pub struct SecStackSinglePageAlloc<Z: MemZeroizer = DefaultMemZeroizer> {
    /// Zeroizer used on deallocation.
    zeroizer: Z,
    /// The number of bytes currently allocated.
    bytes: Cell<usize>,
    /// Page of allocated mlocked memory.
    page: mem::Page,
    // /// Top of the stack, i.e. pointer to the first byte of available memory.
    // stack_ptr: Cell<NonNull<u8>>,
    /// Top of the stack, i.e. offset to the first byte of available memory.
    ///
    /// This is at most the page size.
    /// Page size always fits an `isize` so this can safely be cast to an
    /// `isize`.
    // SAFETY INVARIANT: always a multiple of 8
    // SAFETY INVARIANT: at most page size (`self.page.page_size()`)
    stack_offset: Cell<usize>,
}

impl<Z: MemZeroizer> SecStackSinglePageAlloc<Z> {
    #[cfg(test)]
    /// Panic on inconsistent internal state.
    fn consistency_check(&self) {
        let bytes = self.bytes.get();
        let stack_offset = self.stack_offset.get();
        assert!(
            stack_offset % 8 == 0,
            "safety critical SecStackSinglePageAlloc invariant: offset alignment"
        );
        assert!(
            stack_offset <= self.page.page_size(),
            "safety critical SecStackSinglePageAlloc invariant: offset in page size"
        );
        assert!(
            self.page.as_ptr() as usize % 8 == 0,
            "safety critical SecStackSinglePageAlloc invariant: page alignment"
        );
        assert!(
            bytes <= stack_offset,
            "critical SecStackSinglePageAlloc consistency: allocated bytes in offset"
        );
        assert!(
            bytes % 8 == 0,
            "SecStackSinglePageAlloc consistency: allocated bytes 8 multiple"
        );
    }
}

#[cfg(debug_assertions)]
impl<Z: MemZeroizer> Drop for SecStackSinglePageAlloc<Z> {
    // panic in drop leads to abort, so we better just abort
    // however, abort is only stably available with `std` (not `core`)
    #[cfg(featue = "std")]
    fn drop(&mut self) {
        // check for leaks
        if self.bytes.get() != 0 {
            std::process::abort();
        }
        // check that the entire page contains only zeroized memory
        let page_ptr: *const u8 = self.page.as_ptr();
        for offset in 0..self.page.page_size() {
            // SAFETY: `page_ptr + offset` still points into the memory page, but `offset`
            // doesn't necessarily fit `isize` so we have to use `wrapping_add`
            let byte = unsafe { page_ptr.wrapping_add(offset).read() };
            if byte != 0 {
                std::process::abort();
            }
        }
    }

    #[cfg(not(featue = "std"))]
    fn drop(&mut self) {
        // check for leaks
        debug_assert!(self.bytes.get() == 0);
        // check that the entire page contains only zeroized memory
        let page_ptr: *const u8 = self.page.as_ptr();
        for offset in 0..self.page.page_size() {
            // SAFETY: `page_ptr + offset` still points into the memory page, but `offset`
            // doesn't necessarily fit `isize` so we have to use `wrapping_add`
            let byte = unsafe { page_ptr.wrapping_add(offset).read() };
            assert!(byte == 0);
        }
    }
}

#[cfg(unix)]
impl<Z: MemZeroizer> SecStackSinglePageAlloc<Z> {
    /// Create a new `SecStackSinglePageAlloc` allocator. This allocates one
    /// page of memory to be used by the allocator. This page is only
    /// released once the allocator is dropped.
    ///
    /// # Errors
    /// The function returns an `PageAllocError` if no page could be allocated
    /// by the system or if the page could not be locked. The second can be
    /// caused either by memory starvation of the system or the process
    /// exceeding the amount of memory it is allowed to lock.
    ///
    /// For unprivileged processes amount of memory that locked is very limited
    /// on Linux. A process with `CAP_SYS_RESOURCE` can change the `mlock`
    /// limit using `setrlimit` from libc.
    pub fn new_with_zeroizer(zeroizer: Z) -> Result<Self, mem::PageAllocError> {
        let page = mem::Page::alloc_new_noreserve_mlock()?;
        //let stack_ptr = page.page_ptr_nonnull();
        Ok(Self {
            zeroizer,
            bytes: Cell::new(0),
            page,
            //stack_ptr,
            stack_offset: Cell::new(0),
        })
    }
}

#[cfg(unix)]
impl<Z: MemZeroizer + Default> SecStackSinglePageAlloc<Z> {
    /// Create a new `SecStackSinglePageAlloc` allocator. This allocates one
    /// page of memory to be used by the allocator. This page is only
    /// released once the allocator is dropped.
    ///
    /// # Errors
    /// The function returns an `PageAllocError` if no page could be allocated
    /// by the system or if the page could not be locked. The second can be
    /// caused either by memory starvation of the system or the process
    /// exceeding the amount of memory it is allowed to lock.
    ///
    /// For unprivileged processes amount of memory that locked is very limited
    /// on Linux. A process with `CAP_SYS_RESOURCE` can change the `mlock`
    /// limit using `setrlimit` from libc.
    pub fn new() -> Result<Self, mem::PageAllocError> {
        Self::new_with_zeroizer(Z::default())
    }
}

impl<Z: MemZeroizer> SecStackSinglePageAlloc<Z> {
    /// Returns `true` iff `ptr` points to the final allocation on the memory
    /// page of `self`.
    ///
    /// # SAFETY
    /// This function cannot cause UB on it's own but for the result to be
    /// correct and the function not to panic, the following statements must
    /// hold:
    /// - `ptr` must have been allocated with the allocator `self`
    /// - `rounded_size` must be a size fitting the allocation pointed to by
    ///   `ptr` and must be a multiple of 8 (note that allocation sizes are
    ///   always a multiple of 8)
    ///
    /// In addition, `rounded_size` must be the maximal value satisfying the
    /// second point. If this cannot be assured then the result can be
    /// `false` even if the allocation pointed to by `ptr` is actually the
    /// final allocation.
    fn ptr_is_last_allocation(&self, ptr: NonNull<u8>, rounded_size: usize) -> bool {
        // this doesn't overflow as `ptr` was returned by a previous allocation request
        // so lies in our memory page, so `ptr` is larger than the page pointer
        let alloc_start_offset = ptr.as_ptr() as usize - self.page.as_ptr() as usize;
        // this doesn't overflow since `rounded_size` fits the allocation pointed to by
        // `ptr`
        let alloc_end_offset = alloc_start_offset + rounded_size;
        // `alloc_end_offset` is the stack offset directly after it's allocation
        alloc_end_offset == self.stack_offset.get()
    }

    /// Create a zero-sized allocation.
    ///
    /// # Safety
    /// `align` must be a power of 2
    #[must_use]
    pub unsafe fn allocate_zerosized(align: usize) -> NonNull<[u8]> {
        debug_assert!(align.is_power_of_two());

        // SAFETY: creating a pointer is safe, using it not; `dangling` is non-null
        let dangling: *mut u8 = align as *mut u8;
        let zerosized_slice: *mut [u8] = ptr::slice_from_raw_parts_mut(dangling, 0);
        // SAFETY: zerosized_slice has a non-null pointer part since `align` > 0
        unsafe { NonNull::new_unchecked(zerosized_slice) }
    }

    /// Reallocate allocation into a smaller one.
    ///
    /// This won't try to reuse the existing allocation but forces a new
    /// allocation. Useful if the existing allocation e.g. doesn't have the
    /// correct alignment.
    ///
    /// [`Self::shrink`] falls back to this function if the current allocation
    /// cannot be reused.
    ///
    /// # Safety
    /// Safety contract of this function is identical to that of
    /// [`Allocator::shrink`].
    pub unsafe fn realloc_shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // like the default implementation of `Allocator::shrink` in the standard
        // library
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        let new_ptr = self.allocate(new_layout)?;

        // SAFETY: because `new_layout.size()` must be lower than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for
        // reads and writes for `new_layout.size()` bytes. Also, because the old
        // allocation wasn't yet deallocated, it cannot overlap `new_ptr`. Thus,
        // the call to `copy_nonoverlapping` is safe. The safety contract for
        // `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), nonnull_as_mut_ptr(new_ptr), new_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }

    /// Reallocate allocation into a larger one.
    ///
    /// This won't try to reuse the existing allocation but forces a new
    /// allocation. Useful if the existing allocation e.g. doesn't have the
    /// correct alignment, or is not the last one on the memory page.
    ///
    /// [`Self::grow`] and [`Self::grow_zeroed`] fall back to this function if
    /// the current allocation cannot be reused.
    ///
    /// # Safety
    /// Safety contract of this function is identical to that of
    /// [`Allocator::grow`].
    pub unsafe fn realloc_grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // like the default implementation of `Allocator::grow` in the standard library
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        let new_ptr = self.allocate(new_layout)?;

        // SAFETY: because `new_layout.size()` must be greater than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for
        // reads and writes for `old_layout.size()` bytes. Also, because the old
        // allocation wasn't yet deallocated, it cannot overlap `new_ptr`. Thus,
        // the call to `copy_nonoverlapping` is safe. The safety contract for
        // `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), nonnull_as_mut_ptr(new_ptr), old_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }
}

unsafe impl<Z: MemZeroizer> Allocator for SecStackSinglePageAlloc<Z> {
    // The backing memory is zeroed on deallocation and `mmap` initialises the
    // memory with zeros so every allocation has zeroed memory.
    // We always return a multiple of 8 bytes and a minimal alignment of 8. This
    // allows for fast zeroization and reduces the chance for (external) memory
    // fragmentation, at the cost of increased internal memory fragmentation.
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(layout.align() != 0); // implied by power of 2, but *very important* (safety)
        debug_assert!(layout.align().is_power_of_two());

        // catch zero sized allocations immediately so we do not have to bother with
        // them
        if layout.size() == 0 {
            // SAFETY: `layout.align()` is a power of 2 since that is required by the
            // `Layout` type
            return Ok(unsafe { Self::allocate_zerosized(layout.align()) });
        }
        // if rounding up to a multiple of 8 wraps a usize, the result will be 0 and
        // layout clearly doesn't fit our page, so we return an error
        let rounded_req_size = layout.size().wrapping_add(7usize) & !7usize;
        if unlikely(rounded_req_size == 0) {
            return Err(AllocError);
        }
        // error if we do not have enough space for this allocation
        if rounded_req_size > self.page.page_size() - self.stack_offset.get() {
            return Err(AllocError);
        }

        // SAFETY: `self.stack_offset` is at most the page size so fits an `isize` and
        // the addition does not wrap.
        // SAFETY: `self.stack_offset` is at most the page size so the result of `add`
        // still points into the mapped memory page or one byte after it
        // SAFETY: hence the use of `add` is sound
        let stack_ptr: *mut u8 = unsafe { self.page.as_ptr_mut().add(self.stack_offset.get()) };
        // also the pointer is 8 byte aligned since `self.stack_offset` is a multiple of
        // 8 and the page pointer is page aligned, so also 8 byte aligned

        // we use a minimum alignment of 8 since this allows a fast path for many
        // zeroizers and reduces external memory fragmentation
        if layout.align() <= 8 {
            // fast path for low align
            debug_assert!(
                layout.align() == 1
                    || layout.align() == 2
                    || layout.align() == 4
                    || layout.align() == 8
            );

            let alloc_slice_ptr: *mut [u8] =
                ptr::slice_from_raw_parts_mut(stack_ptr, rounded_req_size);
            // SAFETY: the page pointer is nonnull and the addition doesn't wrap so the
            // result is nonnull
            let alloc_slice_ptr: NonNull<[u8]> = unsafe { NonNull::new_unchecked(alloc_slice_ptr) };

            // SAFETY: rounded_req_size is a multiple of 8 (by rounding) so that
            // `self.stack_offset` stays a multiple of 8
            self.stack_offset
                .set(self.stack_offset.get() + rounded_req_size);

            self.bytes.set(self.bytes.get() + rounded_req_size);
            Ok(alloc_slice_ptr)
        } else {
            // slower path for large align
            // subtract does not wrap since `layout.align()` is a power of 2, hence > 0
            let align_minus_one = layout.align() - 1;
            // first pointer >= `stack_ptr` which is `layout.align()` bytes aligned
            let next_aligned_ptr =
                (stack_ptr as usize).wrapping_add(align_minus_one) & !align_minus_one;
            // if this wraps the address space, then the result is 0 and the layout doesn't
            // fit the remaining memory of our page, so error
            if unlikely(next_aligned_ptr == 0) {
                return Err(AllocError);
            }
            // offset of `next_align_ptr` relative from our base page pointer; doesn't wrap
            // since `next_align_ptr` is higher in the memory than `stack_ptr`
            let next_align_pageoffset = next_aligned_ptr - (self.page.as_ptr() as usize);
            // error if `next_aligned_ptr` falls outside of our page
            if next_align_pageoffset >= self.page.page_size() {
                return Err(AllocError);
            }
            // the new allocation will start at `next_aligned_ptr` and be `rounded_req_size`
            // long error if we do not have enough space for this allocation
            // by the previous branch `self.page.page_size() - next_align_pageoffset` won't
            // wrap (`self.page.page_size() - next_align_pageoffset` is the
            // number of bytes available)
            if rounded_req_size > self.page.page_size() - next_align_pageoffset {
                return Err(AllocError);
            }

            // if we reach here then [next_aligned_ptr .. next_aligned_ptr +
            // rounded_req_size] lies entirely within our memory page
            let alloc_slice_ptr: *mut [u8] =
                ptr::slice_from_raw_parts_mut(next_aligned_ptr as *mut u8, rounded_req_size);
            // SAFETY: the page pointer is nonnull and the addition doesn't wrap so the
            // result is nonnull
            let alloc_slice_ptr: NonNull<[u8]> = unsafe { NonNull::new_unchecked(alloc_slice_ptr) };

            // SAFETY: `rounded_req_size` is a multiple of 8 (by rounding) and
            // `next_align_pageoffset` is so, therefore `self.stack_offset` stays a multiple
            // of 8 SAFETY: `next_align_pageoffset + rounded_req_size` is the
            // first offset after the currently created allocation
            // (`alloc_slice_ptr`)
            self.stack_offset
                .set(next_align_pageoffset + rounded_req_size);

            self.bytes.set(self.bytes.get() + rounded_req_size);
            Ok(alloc_slice_ptr)
        }
    }

    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // zero initialisation doesn't come at a cost, see `allocate_zeroed`
        self.allocate_zeroed(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // catch zero sized allocations immediately so we do not have to bother with
        // them
        if layout.size() == 0 {
            return;
        }

        // `ptr` must be returned by this allocator, so it lies in the currently used
        // part of the memory page
        debug_assert!(self.page.as_ptr() as usize <= ptr.as_ptr() as usize);
        debug_assert!(
            ptr.as_ptr() as usize <= self.page.as_ptr() as usize + self.stack_offset.get()
        );

        // SAFETY: this `rounded_req_size` is identical to the value of
        // `rounded_req_size` in `self.allocate_zeroed` when the block was first
        // allocated since layout must fit the block returned by that function
        // so `layout.size()` now is in the range `layout.size() ..=
        // rounded_req_size` for the values back then this will be important for
        // safety and correct functioning
        let rounded_req_size = layout.size().wrapping_add(7usize) & !7usize;
        // securely wipe the deallocated memory
        // SAFETY: `ptr` is valid for writes of `rounded_req_size` bytes since it was
        // previously successfully allocated (by the safety contract for this
        // function) and not yet deallocated
        // SAFETY: `ptr` is at least `layout.align()` byte aligned and this is a power
        // of two
        unsafe {
            self.zeroizer
                .zeroize_mem_minaligned(ptr.as_ptr(), rounded_req_size, 8);
        }
        // `self.bytes - rounded_req_size` doesn't overflow since the memory has
        // previously been allocated
        self.bytes.set(self.bytes.get() - rounded_req_size);

        // if `self.bytes` is now 0 then this was the last allocation
        // hence we can reset the allocator: reset the stack offset
        if self.bytes.get() == 0 {
            self.stack_offset.set(0);
            return;
        }

        // otherwise, if this allocation was the last one on the stack, rewind the stack
        // offset so we can reuse the memory for later allocation requests

        // this doesn't overflow as `ptr` was returned by a previous allocation request
        // so lies in our memory page, so `ptr` is larger than the page pointer
        let alloc_start_offset = ptr.as_ptr() as usize - self.page.as_ptr() as usize;
        let alloc_end_offset = alloc_start_offset + rounded_req_size;
        // `alloc_end_offset` is the stack offset directly after it's allocation
        if alloc_end_offset == self.stack_offset.get() {
            // SAFETY: `alloc_start_offset` is a multiple of 8 since both `ptr` and the page
            // pointer are 8 byte aligned
            self.stack_offset.set(alloc_start_offset);
        }
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        // catch zero sized allocations immediately so we do not have to bother with
        // them
        if new_layout.size() == 0 {
            // SAFETY: safety contract must be uphold by the caller
            unsafe {
                self.deallocate(ptr, old_layout);
            }
            // SAFETY: `layout.align()` is a power of 2 since that is required by the
            // `Layout` type
            return Ok(unsafe { Self::allocate_zerosized(new_layout.align()) });
        }

        // `ptr` must be returned by this allocator, so it lies in the currently used
        // part of the memory page
        debug_assert!(self.page.as_ptr() as usize <= ptr.as_ptr() as usize);
        debug_assert!(
            ptr.as_ptr() as usize <= self.page.as_ptr() as usize + self.stack_offset.get()
        );

        // check whether the existing allocation has the requested alignment
        if (ptr.as_ptr() as usize) % new_layout.align() == 0 {
            // old allocation has the (new) required alignment
            // we can shrink the allocation in place
            // for a non-final allocation (not last allocation on the memory page) this
            // unfortunately fragments memory; we could as well just not shrink, but we want
            // to zeroize memory as early as possible (and guaranty zeroization)
            // so we do shrink

            // round old layout size to a multiple of 8, since allocation sizes are
            // multiples of 8
            let rounded_size: usize = old_layout.size().wrapping_add(7usize) & !7usize;
            // if the allocation is the final allocation in our memory page, then we can
            // shrink

            // shrink in place
            let new_rounded_size: usize = new_layout.size().wrapping_add(7usize) & !7usize;
            // SAFETY: `ptr` points to an allocation of size at least `rounded_size`, and
            // `new_rounded_size` not larger, so `ptr + new_rounded_size` still points
            // inside our memory page
            // SAFETY: `new_rounded_size` is a multiple of 8 and `ptr` is 8 byte aligned so
            // `new_alloc_end` is so too
            let new_alloc_end: *mut u8 = unsafe { ptr.as_ptr().add(new_rounded_size) };
            // doesn't wrap since `old_layout.size() >= new_layout.size()`, and the
            // inequality is invariant under rounding up to a multiple of 8;
            // also `size_decrease` is therefore a multiple of 8
            let size_decrease: usize = rounded_size - new_rounded_size;
            // securely wipe the deallocated memory
            // SAFETY: `new_alloc_end` is valid for writes of `rounded_size -
            // new_rounded_size` bytes since it is only `new_rounded_size` past
            // `ptr`, which was successfully allocated (by the safety contract
            // for this function) and not yet deallocated
            // SAFETY: `new_alloc_end` is at least 8 byte aligned
            unsafe {
                self.zeroizer
                    .zeroize_mem_minaligned(new_alloc_end, size_decrease, 8);
            }
            // decrement the number of allocated bytes by the allocation size reduction
            self.bytes.set(self.bytes.get() - size_decrease);

            // if the allocation is the final allocation in our memory page, then we can
            // rewind the stack offset to limit memory fragmentation
            // `ptr` is allocated with `self` and `rounded_size` fits it and is a multiple
            // of 8
            if self.ptr_is_last_allocation(ptr, rounded_size) {
                // SAFETY: `size_decrease` is a multiple of 8 so `self.stack_offset` remains so
                self.stack_offset
                    .set(self.stack_offset.get() - size_decrease);
            }

            // create the pointer to the shrunken allocation
            let alloc_slice_ptr: *mut [u8] =
                ptr::slice_from_raw_parts_mut(ptr.as_ptr(), new_rounded_size);
            // SAFETY: `ptr.as_ptr()` is nunnull by the type of `ptr`
            let alloc_slice_ptr: NonNull<[u8]> = unsafe { NonNull::new_unchecked(alloc_slice_ptr) };

            Ok(alloc_slice_ptr)
        } else {
            // wrong alignment, we have to reallocate
            // SAFETY: safety contract must be uphold by the caller
            unsafe { self.realloc_shrink(ptr, old_layout, new_layout) }
        }
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        // catch zero sized allocations immediately so we do not have to bother with
        // them
        if old_layout.size() == 0 {
            // old allocation was zero sized so no need for deallocation
            return self.allocate(new_layout);
        }

        // `ptr` must be returned by this allocator, so it lies in the currently used
        // part of the memory page
        debug_assert!(self.page.as_ptr() as usize <= ptr.as_ptr() as usize);
        debug_assert!(
            ptr.as_ptr() as usize <= self.page.as_ptr() as usize + self.stack_offset.get()
        );

        // check whether the existing allocation has the requested alignment
        if (ptr.as_ptr() as usize) % new_layout.align() == 0 {
            // old allocation has the (new) required alignment
            // if the allocation is the final allocation in our memory page, then we can
            // increase the allocation in-place

            // round old layout size to a multiple of 8, since allocation sizes are
            // multiples of 8
            let rounded_size: usize = old_layout.size().wrapping_add(7usize) & !7usize;
            // `ptr` is allocated with `self` and `rounded_size` fits it and is a multiple
            // of 8
            if self.ptr_is_last_allocation(ptr, rounded_size) {
                // increase allocation in-place

                let new_rounded_size: usize = new_layout.size().wrapping_add(7usize) & !7usize;
                // if this wraps the address space, then the result is 0 and the layout doesn't
                // fit the remaining memory of our page, so error
                if unlikely(new_rounded_size == 0) {
                    return Err(AllocError);
                }

                // this doesn't overflow as `ptr` was returned by a previous allocation request
                // so lies in our memory page, so `ptr` is larger than the page
                // pointer
                let alloc_start_offset = ptr.as_ptr() as usize - self.page.as_ptr() as usize;
                // if the requested allocation size doesn't fit the rest of our page, error
                // the subtraction doesn't wrap since `alloc_start_offset` is the part of the
                // page that is used (without counting the allocation currently
                // being resized)
                if new_rounded_size > self.page.page_size() - alloc_start_offset {
                    return Err(AllocError);
                }

                // if we get here then the resized allocation fits the rest of our memory page
                // this doesn't wrap since `new_layout.size() >= old_layout.size()` so after
                // rounding both to a multiple of 8, `new_rounded_size >= rounded_size`
                // since both values are multiples of 8, `size_increase` is so too
                let size_increase: usize = new_rounded_size - rounded_size;
                // increase the number of allocated bytes by the allocation size increase
                self.bytes.set(self.bytes.get() + size_increase);
                // and the stack offset
                // SAFETY: `size_increase` is a multiple of 8 so `self.stack_offset` remains so
                self.stack_offset
                    .set(self.stack_offset.get() + size_increase);

                // create the pointer to the grown allocation
                let alloc_slice_ptr: *mut [u8] =
                    ptr::slice_from_raw_parts_mut(ptr.as_ptr(), new_rounded_size);
                // SAFETY: `ptr.as_ptr()` is non-null by the type of `ptr`
                let alloc_slice_ptr: NonNull<[u8]> =
                    unsafe { NonNull::new_unchecked(alloc_slice_ptr) };

                return Ok(alloc_slice_ptr);
            }
        }
        // if the alignment of the old allocation is not enough or the allocation is not
        // the last on our memory page, then fall back to making a new
        // allocation and deallocating the older SAFETY: caller must uphold
        // safety contract
        unsafe { self.realloc_grow(ptr, old_layout, new_layout) }
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: caller must uphold safety contract of `Allocator::grow_zeroed`
        unsafe { self.grow_zeroed(ptr, old_layout, new_layout) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zeroize::TestZeroizer;
    use std::mem::drop;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    #[repr(align(16))]
    struct Align16(u128);

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    #[repr(align(16))]
    struct ByteAlign16(u8);

    #[test]
    fn create_consistency() {
        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
    }

    #[test]
    fn box_allocation_8b() {
        use crate::boxed::Box;

        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let _heap_mem = Box::new_in([1u8; 8], &allocator);
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn box_allocation_9b() {
        use crate::boxed::Box;

        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let _heap_mem = Box::new_in([1u8; 9], &allocator);
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn box_allocation_zst() {
        use crate::boxed::Box;

        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let _heap_mem = Box::new_in([(); 8], &allocator);
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn multiple_box_allocations() {
        use crate::boxed::Box;

        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let _heap_mem = Box::new_in([1u8; 9], &allocator);
            allocator.consistency_check();
            {
                let _heap_mem2 = Box::new_in([1u8; 9], &allocator);
                allocator.consistency_check();
            } // drop `_heap_mem2`
            allocator.consistency_check();
            {
                let _heap_mem2prime = Box::new_in([1u8; 9], &allocator);
                allocator.consistency_check();
            } // drop `_heap_mem2prime`
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn multiple_box_allocations_high_align() {
        use crate::boxed::Box;

        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let _heap_mem = Box::new_in([Align16(1); 5], &allocator);
            allocator.consistency_check();
            {
                let _heap_mem2 = Box::new_in([Align16(1); 9], &allocator);
                allocator.consistency_check();
            } // drop `_heap_mem2`
            allocator.consistency_check();
            {
                let _heap_mem2prime = Box::new_in([Align16(1); 2], &allocator);
                allocator.consistency_check();
            } // drop `_heap_mem2prime`
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn multiple_box_allocations_mixed_align() {
        use crate::boxed::Box;

        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let _heap_mem = Box::new_in([1u8; 17], &allocator);
            allocator.consistency_check();
            {
                let _heap_mem2 = Box::new_in([Align16(1); 9], &allocator);
                allocator.consistency_check();
            } // drop `_heap_mem2`
            allocator.consistency_check();
            {
                let _heap_mem2prime = Box::new_in([Align16(1); 2], &allocator);
                allocator.consistency_check();
            } // drop `_heap_mem2prime`
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn many_box_allocations_mixed_align_nonstacked_drop() {
        use crate::boxed::Box;

        let allocator =
            SecStackSinglePageAlloc::<TestZeroizer>::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let heap_mem1 = Box::new_in([Align16(1); 11], &allocator);
            allocator.consistency_check();
            let heap_mem2 = Box::new_in([ByteAlign16(1); 51], &allocator);
            allocator.consistency_check();
            let heap_mem3 = Box::new_in([1u8; 143], &allocator);
            allocator.consistency_check();
            drop(heap_mem3);
            allocator.consistency_check();
            let heap_mem4 = Box::new_in(ByteAlign16(1), &allocator);
            allocator.consistency_check();
            let heap_mem5 = Box::new_in(Align16(1), &allocator);
            allocator.consistency_check();
            drop(heap_mem2);
            allocator.consistency_check();
            drop(heap_mem1);
            allocator.consistency_check();
            drop(heap_mem4);
            allocator.consistency_check();
            drop(heap_mem5);
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_9b() {
        type A = SecStackSinglePageAlloc<TestZeroizer>;

        let allocator: A = SecStackSinglePageAlloc::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let _heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
            allocator.consistency_check();
        } // drop `_heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_grow_repeated() {
        type A = SecStackSinglePageAlloc<TestZeroizer>;

        let allocator: A = SecStackSinglePageAlloc::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let mut heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
            allocator.consistency_check();
            heap_mem.reserve(10);
            allocator.consistency_check();
            heap_mem.reserve(17);
            allocator.consistency_check();
        } // drop `heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_nonfinal_grow() {
        use crate::boxed::Box;
        type A = SecStackSinglePageAlloc<TestZeroizer>;

        let allocator: A = SecStackSinglePageAlloc::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let mut heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
            allocator.consistency_check();
            {
                let heap_mem2 = Box::new_in(37_u64, &allocator);
                allocator.consistency_check();
                heap_mem.reserve(10);
                allocator.consistency_check();
                heap_mem.reserve(17);
                allocator.consistency_check();
            } // drop `heap_mem2`
            allocator.consistency_check();
        } // drop `heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_shrink() {
        type A = SecStackSinglePageAlloc<TestZeroizer>;

        let allocator: A = SecStackSinglePageAlloc::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let mut heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
            allocator.consistency_check();
            heap_mem.push(255);
            allocator.consistency_check();
            heap_mem.shrink_to_fit();
            allocator.consistency_check();
        } // drop `heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_nonfinal_shrink() {
        use crate::boxed::Box;
        type A = SecStackSinglePageAlloc<TestZeroizer>;

        let allocator: A = SecStackSinglePageAlloc::new().expect("allocator creation failed");
        allocator.consistency_check();
        {
            let mut heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
            allocator.consistency_check();
            {
                let heap_mem2 = Box::new_in(37_u64, &allocator);
                allocator.consistency_check();
                heap_mem.push(1);
                allocator.consistency_check();
                heap_mem.shrink_to_fit();
                allocator.consistency_check();
            } // drop `heap_mem2`
            allocator.consistency_check();
        } // drop `heap_mem`
        allocator.consistency_check();
        // drop `allocator`
    }

    #[test]
    fn allocate_zeroed() {
        type A = SecStackSinglePageAlloc<TestZeroizer>;
        let allocator: A = SecStackSinglePageAlloc::new().expect("allocator creation failed");

        let layout = Layout::new::<[u8; 16]>();
        let ptr = allocator
            .allocate_zeroed(layout)
            .expect("allocation failed");
        for i in 0..16 {
            let val: u8 = unsafe { (ptr.as_ptr() as *const u8).add(i).read() };
            assert_eq!(val, 0_u8);
        }
        unsafe {
            allocator.deallocate(ptr.cast(), layout);
        }
    }
}
