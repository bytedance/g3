/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::env;

pub fn check_openssl() {
    let ossl_variant = if env::var("CARGO_FEATURE_VENDORED_OPENSSL").is_ok() {
        "openssl"
    } else if env::var("CARGO_FEATURE_VENDORED_TONGSUO").is_ok() {
        "tongsuo"
    } else if env::var("CARGO_FEATURE_VENDORED_BORINGSSL").is_ok() {
        "boringssl"
    } else if env::var("CARGO_FEATURE_VENDORED_AWS_LC").is_ok() {
        "aws-lc"
    } else {
        "default"
    };
    println!("cargo:rustc-env=G3_OPENSSL_VARIANT={ossl_variant}");
}
