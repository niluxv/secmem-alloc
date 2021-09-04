#![cfg_attr(feature = "nightly_allocator_api", feature(allocator_api))]
#![cfg_attr(feature = "nightly_core_intrinsics", feature(core_intrinsics))]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(future_incompatible, rust_2018_compatibility, unsafe_op_in_unsafe_fn)]
#![deny(rust_2018_idioms)]
#![warn(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
//! `secmem-alloc` is a crate designed allocate private/secret memory. It is
//! intended to be used for storing cryptographic secrets in memory. This crate
//! provides custom allocators using various techniques to improve secrecy of
//! the memory, most notably zeroization on deallocation.
//!
//! # Examples
//! For example, we read in a secret password from standard-in, which we want to
//! zeroize on drop (deallocation). Note that this code does leave the password
//! visible on the prompt; it is only to give an idea of how to use this crate.
//!
//! ```
//! #![feature(allocator_api)]
//! // requires `nightly_allocator_api` crate feature to be enabled and a nightly compiler
//! use secmem_alloc::allocator_api::Allocator;
//! use secmem_alloc::zeroizing_alloc::ZeroizeAlloc;
//! use std::alloc::Global;
//!
//! fn read_password<A: Allocator>(buf: &mut Vec<u8, A>) {
//!     // query password from the user and put it in `buf`
//! }
//!
//! fn main() {
//!     println!("Please enter your password: ");
//!     let mut stdin = std::io::stdin();
//!     let allocator = ZeroizeAlloc::new(Global);
//!     let mut password = Vec::new_in(allocator);
//!     read_password(&mut password);
//!
//!     // use `password` however you like
//!     // you can even grow and shrink the vector `password` and if it needs to be reallocated, the
//!     // old allocation is immediately zeroized
//!
//!     // password is automatically zeroized on drop (deallocation)
//! }
//! ```
//!
//! As a second example assume you have a cryptographic secret key of 256 bytes,
//! which should be zeroized on drop. In addition, we don't want the key to be
//! written to swap.
//!
//! ```
//! // requires no crate features and works on stable
//! // if you enable the `nightly_allocator_api` crate feature, the following line is necessary
//! #![feature(allocator_api)]
//!
//! use secmem_alloc::allocator_api::Allocator;
//! use secmem_alloc::boxed::Box;
//! use secmem_alloc::sec_alloc::SecStackSinglePageAlloc;
//!
//! fn get_secret_key<A: Allocator>(buf: &mut Box<[u8; 256], A>) {
//!     // fill `buf` with the bytes of the secret key
//! }
//!
//! fn main() {
//!     let allocator: SecStackSinglePageAlloc =
//!         SecStackSinglePageAlloc::new().expect("could not create allocator");
//!     let mut key = Box::new_in([0_u8; 256], &allocator);
//!     get_secret_key(&mut key);
//!
//!     // use `key` however you like
//!     // `key` will not be written to swap except possibly on hibernation
//!
//!     // `key` is automatically zeroized on drop (deallocation)
//! }
//! ```
//!
//!
//! # Cargo features
//! - `std` (default): Enable functionality that requires `std`. Currently only
//!   required for `Error` implements and required for tests. This feature is
//!   enabled by default.
//! - `cc` (default): Enable functionality which requires a C compiler. This is
//!   currently only used to implement a secure memory zeroizer
//!   `AsmRepStosZeroizer` written in assembly. This feature is enabled by
//!   default.
//! - `nightly_allocator_api` (requires nightly): Use the nightly allocator api
//!   from the standard library (actually the `core` crate), gated behind the
//!   nightly-only feature `allocator_api`. When disabled, a copy of the
//!   allocator api included in this crate, available through
//!   `secmem_alloc::allocator_api`, will be used. This features requires a
//!   nightly compiler.
//! - `nightly_core_intrinsics` (requires nightly): Use the intrinsics from the
//!   standard library (actually the `core` crate), gated behind the
//!   nightly-only feature `core_intrinsics`. This enables the extremely fast
//!   `VolatileMemsetZeroizer` zeroizer, and various other small optimisations.
//!   This features requires a nightly compiler.
//! - `nightly` (requires nightly): Enable all nightly-only features (i.e. the
//!   above two). Enabling this feature is highly recommended when a nightly
//!   compiler is available. This features requires a nightly compiler.
//! - `dev` (requires nightly): This feature enables all features required to
//!   run the test-suite, and should only be enabled for that purpose. This
//!   features currently requires a nightly compiler.

extern crate alloc;

mod internals;
mod util;

#[cfg(not(feature = "nightly_allocator_api"))]
pub mod allocator_api;
#[cfg(feature = "nightly_allocator_api")]
/// Nightly allocator api, imported from the standard library.
pub mod allocator_api {
    pub use core::alloc::{AllocError, Allocator};
}

pub mod boxed;
pub mod sec_alloc;
pub mod zeroize;
pub mod zeroizing_alloc;

#[cfg(test)]
mod tests {
    /// > Freedom is the freedom to say that two plus two makes four.
    ///
    /// Nineteen Eighty-Four, George Orwell
    #[test]
    fn freedom() {
        assert_ne!(2 + 2, 5);
        assert_eq!(2 + 2, 4);
    }
}
