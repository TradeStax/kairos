//! Build script for kairos-ml
//!
//! This script handles libtorch detection and configuration for the tch crate.
//! LibTorch is the C++ library that PyTorch uses for tensor operations.

use std::env;

fn main() {
    // Let tch handle libtorch detection automatically
    // It will search in common locations or download if needed

    // Set default features for tch if not already set
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LIBTORCH_DIR");
    println!("cargo:rerun-if-env-changed=LIBTORCH_VERSION");

    // Log build info for debugging
    println!("cargo:warning=Building kairos-ml for target: {}", target_os);

    // If LIBTORCH_DIR is set, tell tch where to find it
    if let Ok(libtorch_dir) = env::var("LIBTORCH_DIR") {
        println!("cargo:rustc-env=LIBTORCH_DIR={}", libtorch_dir);
        println!("cargo:warning=Using libtorch from: {}", libtorch_dir);
    }
}
