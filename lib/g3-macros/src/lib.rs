/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

extern crate proc_macro;

use syn::{DeriveInput, parse_macro_input};

mod any_config;

#[proc_macro_derive(AnyConfig, attributes(def_fn, def_async_fn))]
pub fn any_config(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let expanded = any_config::derive(input);
    proc_macro::TokenStream::from(expanded)
}
