/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#![allow(clippy::needless_lifetimes)]
#![allow(clippy::uninlined_format_args)]

pub mod types_capnp {
    #![allow(clippy::extra_unused_type_parameters)]
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/types_capnp.rs"));
}

pub mod proc_capnp {
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/proc_capnp.rs"));
}

pub mod server_capnp {
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/server_capnp.rs"));
}
