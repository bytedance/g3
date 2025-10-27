/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::env;

fn main() {
    g3_build_env::check_basic();
    g3_build_env::check_openssl();
    g3_build_env::check_rustls_provider();

    println!("cargo:rustc-check-cfg=cfg(libressl)");

    if env::var("DEP_OPENSSL_LIBRESSL").is_ok() {
        println!("cargo:rustc-cfg=libressl");
    }

    if env::var("CARGO_FEATURE_QUIC").is_ok() {
        println!("cargo:rustc-env=G3_QUIC_FEATURE=quinn");
    }
}
