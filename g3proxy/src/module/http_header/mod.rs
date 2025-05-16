/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod custom;
mod standard;

pub(crate) use custom::{
    dynamic_egress_info, outgoing_ip, remote_connection_info, set_dynamic_egress_info,
    set_outgoing_ip, set_remote_connection_info, set_upstream_addr, set_upstream_id, upstream_addr,
};
pub(crate) use standard::proxy_authorization_basic_pass;
