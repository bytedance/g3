/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

pub mod request_key {
    pub const HOST: &str = "host";
    pub const SERVICE: &str = "service";
    pub const CERT: &str = "cert";
    pub const USAGE: &str = "usage";
}

pub mod request_key_id {
    pub const HOST: u64 = 1;
    pub const SERVICE: u64 = 2;
    pub const CERT: u64 = 3;
    pub const USAGE: u64 = 4;
}

pub mod response_key {
    pub const HOST: &str = "host";
    pub const SERVICE: &str = "service";
    pub const CERT_CHAIN: &str = "cert";
    pub const PRIVATE_KEY: &str = "key";
    pub const TTL: &str = "ttl";
    pub const USAGE: &str = "usage";
}

pub mod response_key_id {
    pub const HOST: u64 = 1;
    pub const SERVICE: u64 = 2;
    pub const CERT_CHAIN: u64 = 3;
    pub const PRIVATE_KEY: u64 = 4;
    pub const TTL: u64 = 5;
    pub const USAGE: u64 = 6;
}
