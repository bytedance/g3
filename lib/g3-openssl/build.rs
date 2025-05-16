/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::env;

#[allow(clippy::unusual_byte_groupings)]
fn main() {
    println!("cargo:rustc-check-cfg=cfg(ossl300)");

    if let Ok(version) = env::var("DEP_OPENSSL_VERSION_NUMBER") {
        // this will require a dependency on openssl-sys crate
        let version = u64::from_str_radix(&version, 16).unwrap();

        if version >= 0x3_00_00_00_0 {
            println!("cargo:rustc-cfg=ossl300");
        }
    }
}
