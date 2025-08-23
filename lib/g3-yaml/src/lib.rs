/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#[macro_use]
mod macros;

mod callback;
mod hash;
mod hybrid;
mod util;

pub mod humanize;
pub mod key;
pub mod value;

pub use callback::YamlMapCallback;
pub use hash::{
    foreach_kv, get_required as hash_get_required, get_required_str as hash_get_required_str,
};
pub use hybrid::HybridParser;
pub use util::{YamlDocPosition, foreach_doc, load_doc};
