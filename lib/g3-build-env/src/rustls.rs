/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::env;

pub fn check_rustls_provider() {
    if env::var("CARGO_FEATURE_RUSTLS_RING").is_ok() {
        println!("cargo:rustc-env=G3_RUSTLS_PROVIDER=ring");
    }
    if env::var("CARGO_FEATURE_RUSTLS_AWS_LC").is_ok() {
        println!("cargo:rustc-env=G3_RUSTLS_PROVIDER=aws-lc");
    }
}
