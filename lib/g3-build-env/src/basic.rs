/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::env;

pub fn check_basic() {
    let rustc = rustc_version::version_meta().unwrap();
    println!(
        "cargo:rustc-env=G3_BUILD_RUSTC_VERSION={}",
        rustc.short_version_string
    );
    println!("cargo:rustc-env=G3_BUILD_RUSTC_CHANNEL={:?}", rustc.channel);

    println!(
        "cargo:rustc-env=G3_BUILD_HOST={}",
        env::var("HOST").unwrap()
    );
    println!(
        "cargo:rustc-env=G3_BUILD_TARGET={}",
        env::var("TARGET").unwrap()
    );
    println!(
        "cargo:rustc-env=G3_BUILD_PROFILE={}",
        env::var("PROFILE").unwrap()
    );
    println!(
        "cargo:rustc-env=G3_BUILD_OPT_LEVEL={}",
        env::var("OPT_LEVEL").unwrap()
    );
    println!(
        "cargo:rustc-env=G3_BUILD_DEBUG={}",
        env::var("DEBUG").unwrap()
    );

    if let Ok(v) = env::var("G3_PACKAGE_VERSION") {
        println!("cargo:rustc-env=G3_PACKAGE_VERSION={v}");
    }
}
