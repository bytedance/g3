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

use std::path::{Path, PathBuf};

const STATIC_LIB_NAME: &str = "g3-compat";

fn source_dir(os: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("compat-src")
        .join(os)
}

#[cfg(target_os = "linux")]
fn build_linux() {
    let source_dir = source_dir("linux");
    cc::Build::new()
        .cargo_metadata(true)
        .define("_GNU_SOURCE", "1")
        .file(source_dir.join("libc.c"))
        .compile(STATIC_LIB_NAME);
}

#[allow(unused)]
fn build_other() {
    let source_dir = source_dir("other");
    cc::Build::new()
        .cargo_metadata(true)
        .file(source_dir.join("null.c"))
        .compile(STATIC_LIB_NAME);
}

fn main() {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")] {
            build_linux();
        } else {
            build_other();
        }
    }
}
