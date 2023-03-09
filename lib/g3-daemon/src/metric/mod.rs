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

pub const TAG_KEY_DAEMON_GROUP: &str = "daemon_group";

pub const TAG_KEY_STAT_ID: &str = "stat_id";
pub const TAG_KEY_TRANSPORT: &str = "transport";
pub const TAG_KEY_CONNECTION: &str = "connection";
pub const TAG_KEY_REQUEST: &str = "request";

pub const TRANSPORT_TYPE_TCP: &str = "tcp";
pub const TRANSPORT_TYPE_UDP: &str = "udp";

#[derive(Copy, Clone)]
pub enum MetricTransportType {
    Tcp,
    Udp,
}

impl MetricTransportType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            MetricTransportType::Tcp => TRANSPORT_TYPE_TCP,
            MetricTransportType::Udp => TRANSPORT_TYPE_UDP,
        }
    }
}
