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
            .file("schema/user_group.capnp")
            .file("schema/resolver.capnp")
            .file("schema/escaper.capnp")
            .file("schema/server.capnp")
            .run()
            .unwrap();
    }
}
