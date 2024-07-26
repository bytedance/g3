/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
