/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

pub mod request_key {
    pub const IP: &str = "ip";
}

pub mod request_key_id {
    pub const IP: u64 = 1;
}

pub mod response_key {
    pub const IP: &str = "ip";
    pub const TTL: &str = "ttl";
    pub const NETWORK: &str = "network";
    pub const COUNTRY: &str = "country";
    pub const CONTINENT: &str = "continent";
    pub const AS_NUMBER: &str = "as_number";
    pub const ISP_NAME: &str = "isp_name";
    pub const ISP_DOMAIN: &str = "isp_domain";
}

pub mod response_key_id {
    pub const IP: u64 = 1;
    pub const TTL: u64 = 2;
    pub const NETWORK: u64 = 3;
    pub const COUNTRY: u64 = 4;
    pub const CONTINENT: u64 = 5;
    pub const AS_NUMBER: u64 = 6;
    pub const ISP_NAME: u64 = 7;
    pub const ISP_DOMAIN: u64 = 8;
}
