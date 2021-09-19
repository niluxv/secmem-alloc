//! An allocator zeroizing memory on deallocation.
//!
//! This module contains a wrapper for any memory allocator to zeroize memory
//! before deallocation. This allows use both as a [`GlobalAlloc`] and as
//! [`Allocator`].
//!
//! This is safer than zeroizing your secret objects on drop because the
//! allocator approach also zeroizes old memory when the object is only moved
//! in memory but not dropped. This can happen for example when resizing
//! [`Vec`]s.

use crate::allocator_api::{AllocError, Allocator};
use crate::macros::{
    debug_handleallocerror_precondition, debug_handleallocerror_precondition_valid_layout,
    precondition_memory_range,
};
use crate::zeroize::{DefaultMemZeroizer, DefaultMemZeroizerConstructor, MemZeroizer};
use alloc::alloc::handle_alloc_error;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

/// Wrapper around an allocator which zeroizes memory on deallocation. See the
/// module level documentation.
///
/// If debug assertions are enabled, *some* of the safety requirement for using
/// an allocator are checked.
#[derive(Debug, Default)]
pub struct ZeroizeAlloc<BackendAlloc, Z: MemZeroizer = DefaultMemZeroizer> {
    /// Allocator used for the actual allocations.
    backend_alloc: BackendAlloc,
    /// Zeroization stategy for use on deallocation.
    zeroizer: Z,
}

impl<A> ZeroizeAlloc<A> {
    /// Create a zeroizing allocator using `backend_alloc` for allocations and
    /// `zeroizer` to zeroize memory upon deallocation.
    pub const fn new(backend_alloc: A) -> Self {
        Self {
            backend_alloc,
            zeroizer: DefaultMemZeroizerConstructor,
        }
    }
}

impl<A, Z: MemZeroizer> ZeroizeAlloc<A, Z> {
    /// Create a zeroizing allocator using `backend_alloc` for allocations and
    /// `zeroizer` to zeroize memory upon deallocation.
    pub fn with_zeroizer(backend_alloc: A, zeroizer: Z) -> Self {
        Self {
            backend_alloc,
            zeroizer,
        }
    }
}

impl<A, Z: MemZeroizer + Default> ZeroizeAlloc<A, Z> {
    /// Create a zeroizing allocator using `backend_alloc` for allocations and
    /// `zeroizer` to zeroize memory upon deallocation.
    pub fn with_default_zeroizer(backend_alloc: A) -> Self {
        Self::with_zeroizer(backend_alloc, Z::default())
    }
}

unsafe impl<B, Z> GlobalAlloc for ZeroizeAlloc<B, Z>
where
    B: GlobalAlloc,
    Z: MemZeroizer,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // debug assertions
        // SAFETY: the allocator is not allowed to unwind (panic!)
        // check that `layout` is a valid layout
        debug_handleallocerror_precondition_valid_layout!(layout);
        // zero sized allocations are not allowed
        debug_handleallocerror_precondition!(layout.size() != 0, layout);

        // SAFETY: caller must uphold the safety contract of `GlobalAlloc::alloc`.
        unsafe { self.backend_alloc.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // debug assertions
        // SAFETY: the allocator is not allowed to unwind (panic!)
        // null pointers are never allowed
        debug_handleallocerror_precondition!(!ptr.is_null(), layout);
        // check that `layout` is a valid layout
        debug_handleallocerror_precondition_valid_layout!(layout);
        // zero sized allocations are not allowed
        debug_handleallocerror_precondition!(layout.size() != 0, layout);
        // you can't wrap around the address space
        precondition_memory_range!(ptr, layout.size());

        if cfg!(debug_assertions) {
            // you can't wrap around the address space
            if (ptr as usize).checked_add(layout.size()).is_none() {
                handle_alloc_error(layout);
            }
        }

        // securely wipe the deallocated memory
        // SAFETY: `ptr` is valid for writes of `layout.size()` bytes since it was
        // previously successfully allocated (by the safety assumption on this function)
        // and not yet deallocated SAFETY: `ptr` is at least `layout.align()`
        // byte aligned and this is a power of two
        unsafe {
            self.zeroizer
                .zeroize_mem_minaligned(ptr, layout.size(), layout.align());
        }
        // SAFETY: caller must uphold the safety contract of `GlobalAlloc::dealloc`.
        unsafe { self.backend_alloc.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // debug assertions
        // SAFETY: the allocator is not allowed to unwind (panic!)
        // check that `layout` is a valid layout
        debug_handleallocerror_precondition_valid_layout!(layout);
        // zero sized allocations are not allowed
        debug_handleallocerror_precondition!(layout.size() != 0, layout);

        // SAFETY: caller must uphold the safety contract of
        // `GlobalAlloc::alloc_zeroed`.
        unsafe { self.backend_alloc.alloc_zeroed(layout) }
    }

    // We do not use `backend_alloc.realloc` but instead use the default
    // implementation from `std` (actually `core`), so our zeroizing `dealloc`
    // is used. This can degrade performance for 'smart' allocators that would
    // try to reuse the same allocation in realloc.
    // This is the only safe and secure behaviour we can when using an
    // arbitrary backend allocator.
}

unsafe impl<B, Z> Allocator for ZeroizeAlloc<B, Z>
where
    B: Allocator,
    Z: MemZeroizer,
{
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // debug assertions
        // check that `layout` is a valid layout
        debug_handleallocerror_precondition_valid_layout!(layout);

        self.backend_alloc.allocate(layout)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // debug assertions
        // check that `layout` is a valid layout
        debug_handleallocerror_precondition_valid_layout!(layout);

        self.backend_alloc.allocate_zeroed(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // debug assertions
        // check that `layout` is a valid layout
        debug_handleallocerror_precondition_valid_layout!(layout);

        // securely wipe the deallocated memory
        // SAFETY: `ptr` is valid for writes of `layout.size()` bytes since it was
        // previously successfully allocated and not yet deallocated
        // SAFETY: `ptr` is at least `layout.align()` byte aligned and this is a power
        // of two
        unsafe {
            self.zeroizer
                .zeroize_mem_minaligned(ptr.as_ptr(), layout.size(), layout.align());
        }
        // SAFETY: caller must uphold the safety contract of `Allocator::deallocate`
        unsafe { self.backend_alloc.deallocate(ptr, layout) }
    }

    // We do not use `backend_alloc.grow[_zeroed]/shrink` but instead use the
    // default implementation from `std` (actually `core`), so our zeroizing
    // `deallocate` is used. This can degrade performance for 'smart' allocators
    // that would try to reuse the same allocation for such reallocations.
    // This is the only safe and secure behaviour we can when using an
    // arbitrary backend allocator.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zeroize::TestZeroizer;
    use std::alloc::System;

    #[test]
    fn box_allocation_8b() {
        use crate::boxed::Box;

        let allocator = ZeroizeAlloc::with_zeroizer(System, TestZeroizer);
        let _heap_mem = Box::new_in([1u8; 8], &allocator);
        // drop `_heap_mem`
        // drop `allocator`
    }

    #[test]
    fn box_allocation_9b() {
        use crate::boxed::Box;

        let allocator = ZeroizeAlloc::with_zeroizer(System, TestZeroizer);
        let _heap_mem = Box::new_in([1u8; 9], &allocator);
        // drop `_heap_mem`
        // drop `allocator`
    }

    #[test]
    fn box_allocation_zst() {
        use crate::boxed::Box;

        let allocator = ZeroizeAlloc::with_zeroizer(System, TestZeroizer);
        let _heap_mem = Box::new_in([(); 8], &allocator);
        // drop `_heap_mem`
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_9b() {
        let allocator = ZeroizeAlloc::with_zeroizer(System, TestZeroizer);
        let _heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
        // drop `_heap_mem`
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_grow_repeated() {
        let allocator = ZeroizeAlloc::with_zeroizer(System, TestZeroizer);

        let mut heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
        heap_mem.reserve(1);
        heap_mem.reserve(7);
        // drop `heap_mem`
        // drop `allocator`
    }

    #[test]
    fn vec_allocation_shrink() {
        let allocator = ZeroizeAlloc::with_zeroizer(System, TestZeroizer);

        let mut heap_mem = Vec::<u8, _>::with_capacity_in(9, &allocator);
        heap_mem.push(255);
        heap_mem.shrink_to_fit();
        // drop `heap_mem`
        // drop `allocator`
    }

    #[test]
    fn allocate_zeroed() {
        let allocator = ZeroizeAlloc::<_, TestZeroizer>::with_default_zeroizer(System);

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
