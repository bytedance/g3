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

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use chrono::{DateTime, Utc};

use g3_types::net::{EgressInfo, UpstreamAddr};

/// This contains the final chained info about the client request
#[derive(Debug, Clone, Default)]
pub(crate) struct TcpConnectChainedNotes {
    pub(crate) target_addr: Option<SocketAddr>,
    pub(crate) outgoing_addr: Option<SocketAddr>,
}

impl TcpConnectChainedNotes {
    fn reset(&mut self) {
        self.target_addr = None;
        self.outgoing_addr = None;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TcpConnectTaskNotes {
    pub(crate) upstream: UpstreamAddr,
    pub(crate) escaper: String,
    pub(crate) bind: Option<IpAddr>,
    pub(crate) next: Option<SocketAddr>,
    pub(crate) tries: usize,
    pub(crate) local: Option<SocketAddr>,
    pub(crate) expire: Option<DateTime<Utc>>,
    pub(crate) egress: Option<EgressInfo>,
    pub(crate) chained: TcpConnectChainedNotes,
    pub(crate) duration: Duration,
}

impl TcpConnectTaskNotes {
    pub(crate) fn new(upstream: UpstreamAddr) -> Self {
        TcpConnectTaskNotes {
            upstream,
            escaper: String::new(),
            bind: None,
            next: None,
            tries: 0,
            local: None,
            expire: None,
            egress: None,
            chained: Default::default(),
            duration: Duration::ZERO,
        }
    }

    pub(crate) fn empty() -> Self {
        TcpConnectTaskNotes::new(UpstreamAddr::empty())
    }

    #[allow(unused)]
    pub(crate) fn is_empty(&self) -> bool {
        self.upstream.is_empty()
    }

    pub(crate) fn reset_generated(&mut self) {
        self.escaper.clear();
        self.bind = None;
        self.next = None;
        self.tries = 0;
        self.local = None;
        self.expire = None;
        self.egress = None;
        self.chained.reset();
        self.duration = Duration::ZERO;
    }

    pub(crate) fn fill_generated(&mut self, other: &Self) {
        self.escaper.clone_from(&other.escaper);
        self.bind = other.bind;
        self.next = other.next;
        self.tries = other.tries;
        self.local = other.local;
        self.expire = other.expire;
        self.egress = other.egress.clone();
        self.chained.clone_from(&other.chained);
        self.duration = other.duration;
    }
}
