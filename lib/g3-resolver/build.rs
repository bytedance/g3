/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

fn main() {
    println!("cargo:rustc-check-cfg=cfg(cares1_20)");
    println!("cargo:rustc-check-cfg=cfg(cares1_22)");

    #[cfg(feature = "c-ares")]
    if let Ok(version) = std::env::var("DEP_CARES_VERSION_NUMBER") {
        // this will require a dependency on c-ares-sys crate
        let version = u64::from_str_radix(&version, 16).unwrap();

        if version >= 0x1_14_00 {
            println!("cargo:rustc-cfg=cares1_20");
        }

        if version >= 0x1_16_00 {
            println!("cargo:rustc-cfg=cares1_22");
        }
    }
}
