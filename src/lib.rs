// for `volatile_memset`
#![cfg_attr(feature = "nightly_core_intrinsics", feature(core_intrinsics))]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(future_incompatible, unsafe_op_in_unsafe_fn)]
#![warn(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// on nightly, enable some unstable lints
#![cfg_attr(
    feature = "nightly",
    feature(strict_provenance_lints),
    deny(implicit_provenance_casts)
)]
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
//! use std::alloc::{Allocator, Global};
//! use secmem_alloc::zeroizing_alloc::ZeroizeAlloc;
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
//! use core::alloc::Allocator;
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
//! - `nightly_core_intrinsics` (requires nightly): Use the intrinsics from the
//!   standard library (actually the `core` crate), gated behind the
//!   nightly-only feature `core_intrinsics`. This allows for a slightly faster
//!   [`zeroize_mem`] implementation, and various other small optimisations.
//!   This feature requires a nightly compiler.
//! - `nightly` (requires nightly): Enable all nightly-only features (i.e. the
//!   above two). Enabling this feature is highly recommended when a nightly
//!   compiler is available. This feature requires a nightly compiler.
//!
//!
//! # MSRV Policy
//! Since this crate still depends on several nightly features for optimal
//! functioning, the MSRV is expected to track the latest stable relatively
//! closely. Therefore, we might bump the MSRV in patch version bumps. This
//! shouldn't be an issue given the new MSRV aware dependency resolver in
//! cargo. This policy might change as more unstable features get stabilised.

extern crate alloc;

mod internals;
mod util;
mod zeroize;

pub use zeroize::zeroize_mem;

pub mod sec_alloc;
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
