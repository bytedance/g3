/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::env;

fn gen_openssl_flags() {
    println!("cargo:rustc-check-cfg=cfg(libressl)");
    println!("cargo:rustc-check-cfg=cfg(tongsuo)");
    println!("cargo:rustc-check-cfg=cfg(boringssl)");
    println!("cargo:rustc-check-cfg=cfg(awslc)");

    if env::var("DEP_OPENSSL_LIBRESSL").is_ok() {
        println!("cargo:rustc-cfg=libressl");
    }

    if env::var("DEP_OPENSSL_TONGSUO").is_ok() {
        println!("cargo:rustc-cfg=tongsuo");
    }

    if env::var("DEP_OPENSSL_BORINGSSL").is_ok() {
        println!("cargo:rustc-cfg=boringssl");
    }

    if env::var("DEP_OPENSSL_AWSLC").is_ok() {
        println!("cargo:rustc-cfg=awslc");
    }
}

fn main() {
    gen_openssl_flags();
}
