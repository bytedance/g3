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

use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;
use g3_types::net::{SocketBufferConfig, UpstreamAddr};

pub(crate) struct UdpRelayTaskConf<'a> {
    pub(crate) initial_peer: &'a UpstreamAddr,
    pub(crate) sock_buf: SocketBufferConfig,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UdpRelayTaskNotes {
    pub(crate) escaper: NodeName,
    pub(crate) expire: Option<DateTime<Utc>>,
}
