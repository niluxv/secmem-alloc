# Changelog

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
