/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::env;

pub fn check_openssl() {
    let ossl_variant = if env::var("CARGO_FEATURE_VENDORED_OPENSSL").is_ok() {
        "openssl"
    } else if env::var("CARGO_FEATURE_VENDORED_TONGSUO").is_ok() {
        "tongsuo"
    } else if env::var("CARGO_FEATURE_VENDORED_BORINGSSL").is_ok() {
        "boringssl"
    } else if env::var("CARGO_FEATURE_VENDORED_AWS_LC").is_ok() {
        "aws-lc"
    } else {
        "default"
    };
    println!("cargo:rustc-env=G3_OPENSSL_VARIANT={ossl_variant}");
}
