/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#![allow(clippy::needless_lifetimes)]

pub mod types_capnp {
    #![allow(clippy::extra_unused_type_parameters)]
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/types_capnp.rs"));
}

pub mod proc_capnp {
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/proc_capnp.rs"));
}

pub mod user_group_capnp {
    include!(concat!(
        env!("G3_CAPNP_GENERATE_DIR"),
        "/user_group_capnp.rs"
    ));
}

pub mod resolver_capnp {
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/resolver_capnp.rs"));
}

pub mod escaper_capnp {
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/escaper_capnp.rs"));
}

pub mod server_capnp {
    include!(concat!(env!("G3_CAPNP_GENERATE_DIR"), "/server_capnp.rs"));
}
