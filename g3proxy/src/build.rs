/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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

const LUA_FEATURE: Option<&str> = option_env!("G3_LUA_FEATURE");
const PYTHON_FEATURE: Option<&str> = option_env!("G3_PYTHON_FEATURE");
const C_ARES_FEATURE: Option<&str> = option_env!("G3_C_ARES_FEATURE");

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
        println!();
        println!("Compiler: {RUSTC_VERSION} ({RUSTC_CHANNEL})");
    }
    if verbose_level > 1 {
        println!("Host: {BUILD_HOST}, Target: {BUILD_TARGET}");
        println!("Profile: {BUILD_PROFILE}, Opt Level: {BUILD_OPT_LEVEL}, Debug: {BUILD_DEBUG}");
        if let Some(package_version) = PACKAGE_VERSION {
            println!("Package Version: {package_version}");
        }
    }
}
