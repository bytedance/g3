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

use g3_types::net::{SocketBufferConfig, UpstreamAddr};

pub(crate) struct UdpRelayTaskNotes {
    pub(crate) buf_conf: SocketBufferConfig,
    pub(crate) initial_peer: UpstreamAddr,
    pub(crate) escaper: String,
    pub(crate) expire: Option<DateTime<Utc>>,
}

impl UdpRelayTaskNotes {
    pub(crate) fn empty(buf_conf: SocketBufferConfig) -> Self {
        UdpRelayTaskNotes::new(UpstreamAddr::empty(), buf_conf)
    }

    pub(crate) fn new(initial_peer: UpstreamAddr, buf_conf: SocketBufferConfig) -> Self {
        UdpRelayTaskNotes {
            buf_conf,
            initial_peer,
            escaper: String::new(),
            expire: None,
        }
    }
}
