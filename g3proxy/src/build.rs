/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const PKG_NAME: &str = env!("CARGO_PKG_NAME");

const RUSTC_VERSION: &str = env!("G3_BUILD_RUSTC_VERSION");
const RUSTC_CHANNEL: &str = env!("G3_BUILD_RUSTC_CHANNEL");

const BUILD_HOST: &str = env!("G3_BUILD_HOST");
const BUILD_TARGET: &str = env!("G3_BUILD_TARGET");
const BUILD_PROFILE: &str = env!("G3_BUILD_PROFILE");
const BUILD_OPT_LEVEL: &str = env!("G3_BUILD_OPT_LEVEL");
const BUILD_DEBUG: &str = env!("G3_BUILD_DEBUG");

const PACKAGE_VERSION: Option<&str> = option_env!("G3_PACKAGE_VERSION");

const OPENSSL_VARIANT: Option<&str> = option_env!("G3_OPENSSL_VARIANT");
const RUSTLS_PROVIDER: Option<&str> = option_env!("G3_RUSTLS_PROVIDER");

const LUA_FEATURE: Option<&str> = option_env!("G3_LUA_FEATURE");
const PYTHON_FEATURE: Option<&str> = option_env!("G3_PYTHON_FEATURE");
const C_ARES_FEATURE: Option<&str> = option_env!("G3_C_ARES_FEATURE");
const HICKORY_FEATURE: Option<&str> = option_env!("G3_HICKORY_FEATURE");
const QUIC_FEATURE: Option<&str> = option_env!("G3_QUIC_FEATURE");

pub(crate) fn print_version(verbose_level: u8) {
    println!("{PKG_NAME} {VERSION}");
    if verbose_level > 0 {
        print!("Features:");
        if let Some(lua) = LUA_FEATURE {
            print!(" {lua}");
        }
        if let Some(py) = PYTHON_FEATURE {
            print!(" {py}");
        }
        if let Some(c_ares) = C_ARES_FEATURE {
            print!(" {c_ares}");
        }
        if let Some(hickory) = HICKORY_FEATURE {
            print!(" {hickory}");
        }
        if let Some(quic) = QUIC_FEATURE {
            print!(" {quic}");
        }
        println!();
        if let Some(variant) = OPENSSL_VARIANT {
            println!("OpenSSL Variant: {variant}");
        }
        if let Some(provider) = RUSTLS_PROVIDER {
            println!("Rustls Provider: {provider}");
        }
    }
    if verbose_level > 1 {
        println!("Compiler: {RUSTC_VERSION} ({RUSTC_CHANNEL})");
        println!("Host: {BUILD_HOST}, Target: {BUILD_TARGET}");
        println!("Profile: {BUILD_PROFILE}, Opt Level: {BUILD_OPT_LEVEL}, Debug: {BUILD_DEBUG}");
        if let Some(package_version) = PACKAGE_VERSION {
            println!("Package Version: {package_version}");
        }
    }
}
