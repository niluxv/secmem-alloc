# Changelog

## 0.3.0 - 2024-08-20
### Changed
- **Breaking:** Removed `boxed` module. `Box` is now available in the `allocator_api` module.
- **Breaking:** Allocators are no longer generic over a `Zeroizer`, but always use the default.
- On a stable compiler use `allocator-api2` crate instead of our own allocator api clone.
- Improved error messages when page allocation or locking fails.
- Internal: Ported unix support from `libc` to `rustix` crate. This means that in no-std mode this
  crate no longer depends on libc.
- Internal: Ported windows support from `winapi` to `windows` crate.

### Removed
- **Breaking:** Removed `zeroize` module. A single `zeroize_mem` function is now available in the
  crate root instead.

## 0.2.2 - 2024-05-02
### Fixed
- Fixed SSE and AVX zeroizers not properly zeroising the whole memory region under certain
  alignment conditions.
- Fixed potential memory leak on Windows, where the page size was passed to `VirtualAlloc`, where
  it expected a zero value.

## 0.2.1 - 2024-05-01 [YANKED]

__Notice__: Yanked because of the issues described under the 0.2.2 version.

### Fixed
- Fixed __Undefined Behaviour (UB)__ in `SecStackSinglePageAlloc` when not using the
  `nightly_allocator_api` feature.

  The UB would occur when the user deallocates a `secmem_proc::boxed::Box` of a size which
  is not a multiple of 8.

  __Detailed Description__

  The issue is that stds nightly `Allocator` is "magic" w.r.t. the `deallocate` function.
  The pointer input variable (first input) received by the allocator, doesn't have the
  provenance of the pointer that was passed to `deallocate`, but instead the potentially
  larger provenance of the pointer that was returned by `allocate` for this allocation.

  We round up allocation request sizes to multiples of 8, and then in `deallocate` we
  zeroize this full (size multiple of 8) allocation. However, in our stable "clone"
  of `Allocator`, there is no "magic", and the pointer passed to `deallocate` can have
  a provenance to only the number of bytes that were requested in the `allocate` call,
  i.e. not rounded up to a multiple of 8.

## 0.2.0 - 2022-04-12 [YANKED]

__Notice__: Yanked because of the issues described under the 0.2.1 and 0.2.2 versions.

### Added
- Added X86_64 SSE2 and AVX simd zeroizers using inline assembly.
- Added `nightly_stdsimd` and `nightly_strict_provenance` features. Both don't affect
  the library interface currently.

### Changed
- Ported `AsmRepStosZeroizer` to use Rust inline assembly rather than C inline
  assembly so it doesn't require the `cc` feature and a C compiler anymore.
- Changed `MemZeroizer` trait: replaced `zeroize_mem_minaligned` method with new method
  `zeroize_mem_blocks` which takes the logarithm of the align and in addition a
  logarithm of block size as constant generics (such that `len` must be a
  multiple of of this block size, and `ptr` is aligned to the specified align).
- `DefaultMemZeroizer` now uses one of the simd zeroizers when available and no
  libc zeroizer or nightly compiler (`nightly_core_intrinsics` feature) is
  available.

### Removed
- `cc` crate feature, since the C inline assembly has been ported to Rust inline assembly.

## 0.1.2 - 2021-12-01
- Fixed compile error on `no_std` windows
- Added `MAP_NOCORE` flag on page allocation on freebsd-like systems
- Remove int-ptr-casts, making miri pass with `miri-tag-raw-pointers` enabled

## 0.1.1 - 2021-09-21
- Added windows support
- Added [MIRAI](https://github.com/facebookexperimental/MIRAI) annotations to source code
- Excluded unnecessary files from crates package
- Added changelog

## 0.1.0 - 2021-09-04
Initial version
