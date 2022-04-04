# secmem-alloc ![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue) [![secmem-alloc on crates.io](https://img.shields.io/crates/v/secmem-alloc)](https://crates.io/crates/secmem-alloc) [![Source Code Repository](https://img.shields.io/badge/Code-On%20GitHub-blue?logo=GitHub)](https://github.com/niluxv/secmem-alloc)

`secmem-alloc` is a crate designed allocate private/secret memory. It is intended to be used for storing cryptographic secrets in memory. This crate provides custom allocators using various techniques to improve secrecy of the memory, most notably zeroization on deallocation.


## Examples

For example, we read in a secret password from standard-in, which we want to zeroize on drop (deallocation). Note that this code does leave the password visible on the prompt; it is only to give an idea of how to use this crate.


```rust
#![feature(allocator_api)]
// requires `nightly_allocator_api` crate feature to be enabled and a nightly compiler
use secmem_alloc::allocator_api::Allocator;
use secmem_alloc::zeroizing_alloc::ZeroizeAlloc;
use std::alloc::Global;

fn read_password<A: Allocator>(buf: &mut Vec<u8, A>) {
    // query password from the user and put it in `buf`
}

fn main() {
    println!("Please enter your password: ");
    let mut stdin = std::io::stdin();
    let allocator = ZeroizeAlloc::new(Global);
    let mut password = Vec::new_in(allocator);
    read_password(&mut password);

    // use `password` however you like
    // you can even grow and shrink the vector `password` and if it needs to be reallocated, the
    // old allocation is immediately zeroized

    // password is automatically zeroized on drop (deallocation)
}
```

As a second example assume you have a cryptographic secret key of 256 bytes, which should be zeroized on drop. In addition, we don’t want the key to be written to swap.


```rust
// requires no crate features and works on stable
// if you enable the `nightly_allocator_api` crate feature, the following line is necessary
#![feature(allocator_api)]

use secmem_alloc::allocator_api::Allocator;
use secmem_alloc::boxed::Box;
use secmem_alloc::sec_alloc::SecStackSinglePageAlloc;

fn get_secret_key<A: Allocator>(buf: &mut Box<[u8; 256], A>) {
    // fill `buf` with the bytes of the secret key
}

fn main() {
    let allocator: SecStackSinglePageAlloc =
        SecStackSinglePageAlloc::new().expect("could not create allocator");
    let mut key = Box::new_in([0_u8; 256], &allocator);
    get_secret_key(&mut key);

    // use `key` however you like
    // `key` will not be written to swap except possibly on hibernation

    // `key` is automatically zeroized on drop (deallocation)
}
```


## Cargo features

 - `std` (default): Enable functionality that requires `std`. Currently only required for `Error` implements and required for tests. This feature is enabled by default.
 - `nightly_allocator_api` (requires nightly): Use the nightly allocator api from the standard library (actually the `core` crate), gated behind the nightly-only feature `allocator_api`. When disabled, a copy of the allocator api included in this crate, available through `secmem_alloc::allocator_api`, will be used. This features requires a nightly compiler.
 - `nightly_core_intrinsics` (requires nightly): Use the intrinsics from the standard library (actually the `core` crate), gated behind the nightly-only feature `core_intrinsics`. This enables the extremely fast `VolatileMemsetZeroizer` zeroizer, and various other small optimisations. This features requires a nightly compiler.
 - `nightly` (requires nightly): Enable all nightly-only features (i.e. the above two). Enabling this feature is highly recommended when a nightly compiler is available. This features requires a nightly compiler.
 - `dev` (requires nightly): This feature enables all features required to run the test-suite, and should only be enabled for that purpose. This features currently requires a nightly compiler.


## TODOs
 - [ ] write a more comprehensive readme
 - [ ] improve documentation


## Changelog
See `CHANGELOG.md`.


## Documentation
The API documentation of `secmem-alloc` is available at <https://docs.rs/secmem-alloc/*/secmem_alloc/>.
