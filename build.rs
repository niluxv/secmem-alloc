#[cfg(feature = "cc")]
fn main() {
    use std::env;

    // we have a C file to allow assembly on stable
    // only used on x86_64 targets with `ermsb` (cpu feature) support
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")
        .expect("could not find `CARGO_CFG_TARGET_ARCH` environment variable");
    let target_features_str = env::var("CARGO_CFG_TARGET_FEATURE")
        .expect("could not find `CARGO_CFG_TARGET_FEATURE` environment variable");
    let target_features = target_features_str.split(',');
    let mut target_feature_ermsb = false;
    for tf in target_features {
        if tf == "ermsb" {
            target_feature_ermsb = true;
            break;
        }
    }
    if target_arch == "x86_64" && target_feature_ermsb {
        // Rebuild if C sources change
        println!("cargo:rerun-if-changed=src/internals/zeroize_asm_c_impl_ermsb.c");
        println!("cargo:rerun-if-env-changed=CC");
        println!("cargo:rerun-if-env-changed=CFLAGS");
        cc::Build::new()
            .file("src/internals/zeroize_asm_c_impl_ermsb.c")
            .opt_level(3)
            .compile("zeroize_asm_c_impl_ermsb");
    }
}

#[cfg(not(feature = "cc"))]
fn main() {}
