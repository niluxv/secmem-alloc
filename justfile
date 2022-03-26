#!/usr/bin/env just --justfile

check:
    cargo +stable check
    cargo +nightly check --all-features

test:
    cargo +nightly test --no-default-features --features dev
    cargo +nightly test --all-features

miri:
    env MIRIFLAGS="-Zmiri-check-number-validity" cargo +nightly miri test --all-features

miri-raw-ptr:
    env MIRIFLAGS="-Zmiri-tag-raw-pointers -Zmiri-check-number-validity" cargo +nightly miri test --all-features

full-test: test
    env CC="clang" env CFLAGS="-fsanitize=address -fno-omit-frame-pointer" env RUSTFLAGS="-C target-cpu=native -Z sanitizer=address" cargo +nightly test -Z build-std --target x86_64-unknown-linux-gnu --tests --all-features
    env CC="clang" env CFLAGS="-fsanitize=memory -fno-omit-frame-pointer" env RUSTFLAGS="-C target-cpu=native -Z sanitizer=memory" cargo +nightly test -Z build-std --target x86_64-unknown-linux-gnu --tests --all-features

doc:
    cargo +nightly doc --all-features

doc-open:
    cargo +nightly doc --all-features --open

fmt:
    cargo +nightly fmt

fmt-check:
    cargo +nightly fmt -- --check

clippy:
    cargo +nightly clippy --all-features

full-check: check full-test doc clippy fmt-check miri miri-raw-ptr

bench:
    env RUSTFLAGS="-C target-cpu=native" cargo +nightly bench --all-features

code-cov:
    cargo +nightly tarpaulin --ignore-tests --all-features --out Html

clean:
    cargo clean

dist-clean: clean
    rm Cargo.lock
    rm tarpaulin-report.html

generate-readme:
    cargo doc2readme

mirai:
    env MIRAI_FLAGS="--diag=library" cargo mirai
