/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::env;
use std::path::PathBuf;

fn main() {
    if env::var("G3_CAPNP_USE_PRECOMPILED").is_ok() {
        let gen_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("gen");
        println!(
            "cargo:rustc-env=G3_CAPNP_GENERATE_DIR={}",
            gen_dir.display()
        );
    } else {
        println!(
            "cargo:rustc-env=G3_CAPNP_GENERATE_DIR={}",
            env::var("OUT_DIR").unwrap()
        );
        capnpc::CompilerCommand::new()
            .src_prefix("schema")
            .file("schema/types.capnp")
            .file("schema/proc.capnp")
            .run()
            .unwrap();
    }
}
