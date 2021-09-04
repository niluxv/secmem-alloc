//! Module providing a simple replacement for [`std::boxed::Box`] with allocator
//! support.
//!
//! # Motivation
//! - The allocator api of [`std::boxed::Box`] is still unstable (at the time of
//!   writing). The [`Box`] provided by this module can be used on stable with
//!   allocators.
//! - [Issue #78459](https://github.com/rust-lang/rust/issues/78459) prevents
//!   the use of non-zero sized allocators with [`std::boxed::Box`] even on
//!   nightly.
// some code and documentation is copied from the standard library

use crate::allocator_api::{AllocError, Allocator};
use alloc::alloc::handle_alloc_error;
use core::alloc::Layout;
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

/// A replacement for [`std::boxed::Box`] that works with custom allocators.
///
/// See the module-level documentation for more.
pub struct Box<T: ?Sized, A: Allocator> {
    /// Pointer to the inner value, allocated with `self.alloc`.
    // Safety: must always point to a valid instance of `T`.
    ptr: NonNull<T>,
    // we own an instance of type `T`
    _phantom_heapmem: PhantomData<T>,
    /// Allocator used for heap allocation
    alloc: A,
}

impl<T: ?Sized, A: Allocator> Box<T, A> {
    /// Create [`Box`] from a pointer and an allocator.
    ///
    /// # Safety
    /// - `ptr` has to be allocated using the allocator `alloc` (and not yet
    ///   deallocated)
    /// - `ptr` must point to a valid instance of `T` (otherwise using e.g.
    ///   [`Deref::deref`] on the resulting [`Box`] is unsound)
    /// - in particular `ptr` must point to an allocation that fits
    ///   `Layout::for_value(*ptr)`
    unsafe fn from_raw_parts(ptr: NonNull<T>, alloc: A) -> Self {
        Self {
            ptr,
            alloc,
            _phantom_heapmem: PhantomData::<T>,
        }
    }

    /// Destruct a [`Box`] into the pointer and allocator without dropping the
    /// [`Box`].
    fn into_raw_parts(self) -> (NonNull<T>, A) {
        let ptr = self.ptr;
        let me = ManuallyDrop::new(self);
        let alloc_ptr = &me.deref().alloc as *const A;
        // SAFETY: `alloc_ptr` is valid for reads, properly aligned, initialised...
        // SAFETY: the contents of `me` are never dropped so `alloc` can be safely
        // dropped later
        let alloc = unsafe { alloc_ptr.read() };
        (ptr, alloc)
    }
}

// documentation and implementations copied from the standard library
// Copyright (c) 2021 rust standard library contributors
// slight modifications to accomodate for missing APIs, different `Box`
// definition
impl<T, A: Allocator> Box<T, A> {
    /// Allocates memory in the given allocator then places `x` into it.
    ///
    /// This doesn't actually allocate if `T` is zero-sized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use secmem_alloc::boxed::Box;
    /// use std::alloc::System;
    ///
    /// let five = Box::new_in(5, System);
    /// ```
    #[inline]
    pub fn new_in(x: T, alloc: A) -> Self {
        let mut boxed = Self::new_uninit_in(alloc);
        unsafe {
            boxed.as_mut_ptr().write(x);
            boxed.assume_init()
        }
    }

    /// Allocates memory in the given allocator then places `x` into it,
    /// returning an error if the allocation fails
    ///
    /// This doesn't actually allocate if `T` is zero-sized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use secmem_alloc::boxed::Box;
    /// use std::alloc::System;
    ///
    /// let five = Box::try_new_in(5, System)?;
    /// # Ok::<(), core::alloc::AllocError>(())
    /// ```
    #[inline]
    pub fn try_new_in(x: T, alloc: A) -> Result<Self, AllocError> {
        let mut boxed = Self::try_new_uninit_in(alloc)?;
        unsafe {
            boxed.as_mut_ptr().write(x);
            Ok(boxed.assume_init())
        }
    }

    /// Constructs a new box with uninitialized contents in the provided
    /// allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use secmem_alloc::boxed::Box;
    /// use std::alloc::System;
    ///
    /// let mut five = Box::<u32, _>::new_uninit_in(System);
    ///
    /// let five = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    pub fn new_uninit_in(alloc: A) -> Box<MaybeUninit<T>, A> {
        let layout = Layout::new::<MaybeUninit<T>>();
        // NOTE: Prefer match over unwrap_or_else since closure sometimes not
        // inlineable. That would make code size bigger.
        match Box::try_new_uninit_in(alloc) {
            Ok(m) => m,
            Err(_) => handle_alloc_error(layout),
        }
    }

    /// Constructs a new box with uninitialized contents in the provided
    /// allocator, returning an error if the allocation fails
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use secmem_alloc::boxed::Box;
    /// use std::alloc::System;
    ///
    /// let mut five = Box::<u32, _>::try_new_uninit_in(System)?;
    ///
    /// let five = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5);
    /// # Ok::<(), core::alloc::AllocError>(())
    /// ```
    pub fn try_new_uninit_in(alloc: A) -> Result<Box<MaybeUninit<T>, A>, AllocError> {
        let layout = Layout::new::<MaybeUninit<T>>();
        let ptr: NonNull<MaybeUninit<T>> = alloc.allocate(layout)?.cast();
        unsafe { Ok(Box::from_raw_parts(ptr, alloc)) }
    }
}

// documentation and implementations copied from the standard library
// Copyright (c) 2021 rust standard library contributors
// slight modifications to accomodate for missing APIs, different `Box`
// definition
impl<T, A: Allocator> Box<MaybeUninit<T>, A> {
    /// Converts to `Box<T, A>`.
    ///
    /// # Safety
    ///
    /// As with [`MaybeUninit::assume_init`],
    /// it is up to the caller to guarantee that the value
    /// really is in an initialized state.
    /// Calling this when the content is not yet fully initialized
    /// causes immediate undefined behavior.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use secmem_alloc::boxed::Box;
    /// use std::alloc::System;
    ///
    /// let mut five = Box::<u32, _>::new_uninit_in(System);
    ///
    /// let five: Box<u32, _> = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    #[inline]
    pub unsafe fn assume_init(self) -> Box<T, A> {
        let (ptr, alloc) = Box::into_raw_parts(self);
        let ptr_init: NonNull<T> = ptr.cast();
        unsafe { Box::from_raw_parts(ptr_init, alloc) }
    }
}

impl<T: ?Sized, A: Allocator> Deref for Box<T, A> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: `self.ptr` always points to a valid instance of `T`
        unsafe { &*self.ptr.as_ptr() }
    }
}

impl<T: ?Sized, A: Allocator> DerefMut for Box<T, A> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: `self.ptr` always points to a valid instance of `T`
        unsafe { &mut *self.ptr.as_ptr() }
    }
}

impl<T: ?Sized, A: Allocator> Drop for Box<T, A> {
    fn drop(&mut self) {
        // obtain the Layout of the value stored in this Box
        let ref_to_inner: &T = self.deref();
        let layout = Layout::for_value::<T>(ref_to_inner);
        // `self.ptr` points to an allocation that fits `layout`

        // SAFETY: `self.ptr.as_ptr()` is valid for reads and writes, properly aligned
        unsafe {
            self.ptr.as_ptr().drop_in_place();
        }
        // SAFETY: from now on it is unsound to dereference `self.ptr` (hence `self`)

        // deallocate memory
        let ptr: NonNull<u8> = self.ptr.cast();
        // SAFETY: `self.ptr` was allocated with allocator `self.alloc` and fits
        // `layout`
        unsafe {
            self.alloc.deallocate(ptr, layout);
        }
        // `self.ptr` is now dangling, but this is sound since `NonNull<T>` is
        // not `Drop` `self.alloc` is dropped automatically
    }
}

#[cfg(test)]
mod tests {
    use super::Box;
    use std::alloc::System;
    use std::mem::MaybeUninit;

    #[test]
    fn new_in() {
        let boxed = Box::new_in([37; 256], System);
        assert_eq!(*boxed, [37; 256]);
    }

    #[test]
    fn try_new_in() {
        let boxed = Box::try_new_in([37; 256], System).expect("error creating box");
        assert_eq!(*boxed, [37; 256]);
    }

    #[test]
    fn uninit_initialise() {
        let mut boxed: Box<MaybeUninit<[u8; 256]>, System> =
            Box::<[u8; 256], _>::new_uninit_in(System);
        unsafe {
            // initialise `boxed`
            boxed.as_mut_ptr().write([37; 256]);
        }
        // SAFETY: `boxed` is now initialised
        let boxed: Box<[u8; 256], System> = unsafe { boxed.assume_init() };
        assert_eq!(*boxed, [37; 256]);
    }
}
