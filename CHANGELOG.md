# Changelog

## 0.2.0 - 2022-04-12
### Added
- X86_64 SSE2 and AVX simd zeroizers using inline assembly.
- `nightly_stdsimd` and `nightly_strict_provenance` features. Both don't affect
  the library interface currently.

### Changed
- Ported `AsmRepStosZeroizer` to use Rust inline assembly rather than C inline
  assembly so it doesn't require the `cc` feature and a C compiler anymore.
- `MemZeroizer` trait, replaced `zeroize_mem_minaligned` method with new method
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
