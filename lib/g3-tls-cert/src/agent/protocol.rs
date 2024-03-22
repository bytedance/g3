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
    pub const HOST: &str = "host";
    pub const SERVICE: &str = "service";
    pub const CERT: &str = "cert";
}

pub mod request_key_id {
    pub const HOST: u64 = 1;
    pub const SERVICE: u64 = 2;
    pub const CERT: u64 = 3;
}

pub mod response_key {
    pub const HOST: &str = "host";
    pub const SERVICE: &str = "service";
    pub const CERT_CHAIN: &str = "cert";
    pub const PRIVATE_KEY: &str = "key";
    pub const TTL: &str = "ttl";
}

pub mod response_key_id {
    pub const HOST: u64 = 1;
    pub const SERVICE: u64 = 2;
    pub const CERT_CHAIN: u64 = 3;
    pub const PRIVATE_KEY: u64 = 4;
    pub const TTL: u64 = 5;
}
