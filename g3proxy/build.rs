/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::env;

fn main() {
    g3_build_env::check_basic();
    g3_build_env::check_openssl();
    g3_build_env::check_rustls_provider();

    if env::var("CARGO_FEATURE_LUA").is_ok() {
        if env::var("CARGO_FEATURE_LUA51").is_ok() {
            println!("cargo:rustc-env=G3_LUA_FEATURE=lua51");
        } else if env::var("CARGO_FEATURE_LUA53").is_ok() {
            println!("cargo:rustc-env=G3_LUA_FEATURE=lua53");
        } else if env::var("CARGO_FEATURE_LUA54").is_ok() {
            println!("cargo:rustc-env=G3_LUA_FEATURE=lua54");
        } else if env::var("CARGO_FEATURE_LUAJIT").is_ok() {
            println!("cargo:rustc-env=G3_LUA_FEATURE=luajit");
        }
    }

    if env::var("CARGO_FEATURE_PYTHON").is_ok() {
        println!("cargo:rustc-env=G3_PYTHON_FEATURE=python");
    }

    if env::var("CARGO_FEATURE_C_ARES").is_ok() {
        println!("cargo:rustc-env=G3_C_ARES_FEATURE=c-ares");
    }

    if env::var("CARGO_FEATURE_QUIC").is_ok() {
        println!("cargo:rustc-env=G3_QUIC_FEATURE=quinn");
    }
}
