// https://github.com/rust-lang/rust/issues/32838
#![cfg_attr(feature = "nightly_allocator_api", feature(allocator_api))]
// for `volatile_memset`
#![cfg_attr(feature = "nightly_core_intrinsics", feature(core_intrinsics))]
// https://github.com/rust-lang/rust/issues/111137
#![cfg_attr(feature = "nightly_stdsimd", feature(stdarch_x86_avx512))]
// https://github.com/rust-lang/rust/issues/95228
#![cfg_attr(feature = "nightly_strict_provenance", feature(strict_provenance))]
#![cfg_attr(
    feature = "nightly_strict_provenance",
    deny(fuzzy_provenance_casts, lossy_provenance_casts)
)]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(rust_2018_compatibility, unsafe_op_in_unsafe_fn)]
#![deny(future_incompatible, rust_2018_idioms)]
#![warn(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// FIXME: disable when strict provenance is stabilised
#![allow(unstable_name_collisions)]
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
//! use secmem_alloc::allocator_api::{Allocator, Global, Vec};
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
//! // requires no crate features and works on stable
//! // if you enable the `nightly_allocator_api` crate feature, the following line is necessary
//! #![feature(allocator_api)]
//!
//! use secmem_alloc::allocator_api::{Allocator, Box};
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
//! - `nightly_allocator_api` (requires nightly): Use the nightly allocator api
//!   from the standard library (actually the `core` crate), gated behind the
//!   nightly-only feature `allocator_api`. When disabled, a copy of the
//!   allocator api included in this crate, available through
//!   `secmem_alloc::allocator_api`, will be used. This feature requires a
//!   nightly compiler.
//! - `nightly_core_intrinsics` (requires nightly): Use the intrinsics from the
//!   standard library (actually the `core` crate), gated behind the
//!   nightly-only feature `core_intrinsics`. This enables the extremely fast
//!   `VolatileMemsetZeroizer` zeroizer, and various other small optimisations.
//!   This feature requires a nightly compiler.
//! - `nightly_stdsimd` (requires nightly): Required for avx512 simd API in the
//!   standard libary, but currently unused. This feature requires a nightly
//!   compiler.
//! - `nightly_strict_provenance` (requires nightly): Enable strict provenance
//!   lints and (mostly) use strict provenance API provided by the standard
//!   library instead of the one from `sptr`. (Will still depend on and in a few
//!   places even use `sptr`.)
//! - `nightly` (requires nightly): Enable all nightly-only features (i.e. the
//!   above two). Enabling this feature is highly recommended when a nightly
//!   compiler is available. This feature requires a nightly compiler.
//! - `dev` (requires nightly): This feature enables all features required to
//!   run the test-suite, and should only be enabled for that purpose. This
//!   feature currently requires a nightly compiler.

extern crate alloc;

/// Re-exports the most important items of the [`allocator-api2` crate].
///
/// [`allocator-api2` crate]: https://crates.io/crates/allocator-api2
pub mod allocator_api {
    pub use allocator_api2::alloc::{Allocator, Global};
    pub use allocator_api2::boxed::Box;
    pub use allocator_api2::vec::Vec;
}

mod internals;
mod macros;
mod util;

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
