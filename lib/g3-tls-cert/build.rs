/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::env;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(osslconf, values(\"OPENSSL_NO_SM2\"))");

    if let Ok(vars) = env::var("DEP_OPENSSL_CONF") {
        for var in vars.split(',') {
            println!("cargo:rustc-cfg=osslconf=\"{var}\"");
        }
    }
}
