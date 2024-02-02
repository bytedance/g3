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
use std::path::Path;

use bindgen::MacroTypeVariation;

// Keep in sync with the list in include/openssl/opensslconf.h
const OSSL_CONF_DEFINES: &[&str] = &[
    "OPENSSL_NO_ASYNC",
    "OPENSSL_NO_BF",
    "OPENSSL_NO_BLAKE2",
    "OPENSSL_NO_BUF_FREELISTS",
    "OPENSSL_NO_CAMELLIA",
    "OPENSSL_NO_CAPIENG",
    "OPENSSL_NO_CAST",
    "OPENSSL_NO_CMS",
    "OPENSSL_NO_COMP",
    "OPENSSL_NO_CT",
    "OPENSSL_NO_DANE",
    "OPENSSL_NO_DEPRECATED",
    "OPENSSL_NO_DGRAM",
    "OPENSSL_NO_DYNAMIC_ENGINE",
    "OPENSSL_NO_EC_NISTP_64_GCC_128",
    "OPENSSL_NO_EC2M",
    "OPENSSL_NO_EGD",
    "OPENSSL_NO_ENGINE",
    "OPENSSL_NO_GMP",
    "OPENSSL_NO_GOST",
    "OPENSSL_NO_HEARTBEATS",
    "OPENSSL_NO_HW",
    "OPENSSL_NO_IDEA",
    "OPENSSL_NO_JPAKE",
    "OPENSSL_NO_KRB5",
    "OPENSSL_NO_MD2",
    "OPENSSL_NO_MDC2",
    "OPENSSL_NO_OCB",
    "OPENSSL_NO_OCSP",
    "OPENSSL_NO_RC2",
    "OPENSSL_NO_RC5",
    "OPENSSL_NO_RFC3779",
    "OPENSSL_NO_RIPEMD",
    "OPENSSL_NO_RMD160",
    "OPENSSL_NO_SCTP",
    "OPENSSL_NO_SEED",
    "OPENSSL_NO_SM2",
    "OPENSSL_NO_SM3",
    "OPENSSL_NO_SM4",
    "OPENSSL_NO_SRP",
    "OPENSSL_NO_SSL_TRACE",
    "OPENSSL_NO_SSL2",
    "OPENSSL_NO_SSL3",
    "OPENSSL_NO_SSL3_METHOD",
    "OPENSSL_NO_STATIC_ENGINE",
    "OPENSSL_NO_STORE",
    "OPENSSL_NO_WHIRLPOOL",
];

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = Path::new(&crate_dir).join("../../");
    let include_dir = src_dir.join("boringssl/include");
    let boringssl_src_dir = src_dir.join("third_party/boringssl");

    let out_dir = env::var("OUT_DIR").unwrap();
    let bindgen_file = Path::new(&out_dir).join("bindgen.rs");

    let target = env::var("TARGET").unwrap();

    // compile rust_wrapper
    println!("cargo:rerun-if-changed=rust_wrapper.c");
    println!("cargo:rerun-if-changed=rust_wrapper.h");
    cc::Build::new()
        .cargo_metadata(true)
        .include(&include_dir)
        .file("rust_wrapper.c")
        .compile("rustc_wrapper");

    // bindgen
    let binding = bindgen::Builder::default()
        .header("wrapper.h")
        .derive_default(false)
        .enable_function_attribute_detection()
        .use_core()
        .default_macro_constant_type(MacroTypeVariation::Signed)
        .rustified_enum("point_conversion_form_t")
        .clang_args([
            format!("-I{}", include_dir.display()),
            format!("--target={target}"),
        ])
        .generate()
        .expect("unable to generate binding for BoringSSL");
    binding
        .write_to_file(&bindgen_file)
        .expect("failed to write bindgen file");
    println!(
        "cargo:rustc-env=BSSL_BINDGEN_RS_FILE={}",
        bindgen_file.display()
    );

    // build BoringSSL code
    println!("cargo:rerun-if-changed={}", boringssl_src_dir.display());
    let boringssl_build_dir = cmake::Config::new(boringssl_src_dir)
        .build_target("crypto")
        .build_target("ssl")
        .build();

    // set link options
    println!(
        "cargo:rustc-link-search=native={}/build",
        boringssl_build_dir.display()
    );
    println!("cargo:rustc-link-lib=static=crypto");
    println!("cargo:rustc-link-lib=static=ssl");

    // OSSL CONF
    println!("cargo:conf={}", OSSL_CONF_DEFINES.join(","));
}
