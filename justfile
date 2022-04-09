#!/usr/bin/env just --justfile

check:
    cargo +stable check
    cargo +nightly check --all-features

test:
    cargo +nightly test --no-default-features --features dev
    env RUSTFLAGS="-C target-cpu=native" cargo +nightly test --all-features

miri:
    env MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-check-number-validity" cargo +nightly miri test --all-features

test-address-sanatize:
    env CC="clang" env CFLAGS="-fsanitize=address -fno-omit-frame-pointer" env RUSTFLAGS="-C target-cpu=native -Z sanitizer=address" cargo +nightly test -Z build-std --target x86_64-unknown-linux-gnu --tests --all-features

test-memory-sanatize:
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

full-check: check test test-memory-sanatize doc clippy fmt-check miri

bench:
    env RUSTFLAGS="-C target-cpu=native" cargo +nightly criterion --all-features

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
    env MIRAI_FLAGS="--diag=library" cargo mirai --features dev
