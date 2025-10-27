/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::env;

pub fn check_openssl() {
    println!("cargo:rustc-check-cfg=cfg(libressl)");
    println!("cargo:rustc-check-cfg=cfg(tongsuo)");
    println!("cargo:rustc-check-cfg=cfg(boringssl)");
    println!("cargo:rustc-check-cfg=cfg(awslc)");

    if env::var("DEP_OPENSSL_LIBRESSL").is_ok() {
        println!("cargo:rustc-cfg=libressl");
        println!("cargo:rustc-env=G3_OPENSSL_VARIANT=LibreSSL");
        return;
    }

    if env::var("DEP_OPENSSL_TONGSUO").is_ok() {
        println!("cargo:rustc-cfg=tongsuo");
        println!("cargo:rustc-env=G3_OPENSSL_VARIANT=Tongsuo");
        return;
    }

    if env::var("DEP_OPENSSL_BORINGSSL").is_ok() {
        println!("cargo:rustc-cfg=boringssl");
        println!("cargo:rustc-env=G3_OPENSSL_VARIANT=BoringSSL");
        return;
    }

    if env::var("DEP_OPENSSL_AWSLC").is_ok() {
        println!("cargo:rustc-cfg=awslc");
        println!("cargo:rustc-env=G3_OPENSSL_VARIANT=AWS-LC");
        return;
    }

    if env::var("DEP_OPENSSL_AWSLC_FIPS").is_ok() {
        println!("cargo:rustc-cfg=awslc");
        println!("cargo:rustc-cfg=awslc_fips");
        println!("cargo:rustc-env=G3_OPENSSL_VARIANT=AWS-LC-FIPS");
        return;
    }

    println!("cargo:rustc-env=G3_OPENSSL_VARIANT=OpenSSL");
}
