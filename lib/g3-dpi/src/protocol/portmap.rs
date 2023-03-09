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

use fnv::FnvHashMap;

use super::MaybeProtocol;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolPortMapValue {
    check_ssl: bool,
    protocols: Vec<MaybeProtocol>,
}

impl Default for ProtocolPortMapValue {
    fn default() -> Self {
        ProtocolPortMapValue {
            check_ssl: false,
            protocols: Vec::with_capacity(2),
        }
    }
}

impl ProtocolPortMapValue {
    pub(crate) fn check_ssl(&self) -> bool {
        self.check_ssl
    }

    #[inline]
    pub(crate) fn protocols(&self) -> &[MaybeProtocol] {
        self.protocols.as_slice()
    }

    fn insert(&mut self, protocol: MaybeProtocol) {
        let p = match protocol {
            MaybeProtocol::Ssl => {
                self.check_ssl = true;
                return;
            }
            MaybeProtocol::Https => {
                self.check_ssl = true;
                MaybeProtocol::Http
            }
            MaybeProtocol::Pop3s => {
                self.check_ssl = true;
                MaybeProtocol::Pop3
            }
            MaybeProtocol::Nntps => {
                self.check_ssl = true;
                MaybeProtocol::Nntp
            }
            MaybeProtocol::Imaps => {
                self.check_ssl = true;
                MaybeProtocol::Imap
            }
            MaybeProtocol::Rtsps => {
                self.check_ssl = true;
                MaybeProtocol::Rtsp
            }
            MaybeProtocol::SecureMqtt => {
                self.check_ssl = true;
                MaybeProtocol::Mqtt
            }
            MaybeProtocol::Rtmps => {
                self.check_ssl = true;
                MaybeProtocol::Rtmp
            }
            p => p,
        };
        if !self.protocols.contains(&p) {
            self.protocols.push(p);
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProtocolPortMap {
    inner: FnvHashMap<u16, ProtocolPortMapValue>,
}

impl ProtocolPortMap {
    pub fn empty() -> Self {
        ProtocolPortMap {
            inner: FnvHashMap::default(),
        }
    }

    pub fn insert_batch(&mut self, port: u16, protocols: &[MaybeProtocol]) {
        let node = self
            .inner
            .entry(port)
            .or_insert_with(ProtocolPortMapValue::default);
        for protocol in protocols {
            node.insert(*protocol);
        }
    }

    pub fn insert(&mut self, port: u16, protocol: MaybeProtocol) {
        let node = self
            .inner
            .entry(port)
            .or_insert_with(ProtocolPortMapValue::default);
        node.insert(protocol);
    }

    pub fn tcp_server() -> Self {
        let mut map = ProtocolPortMap::empty();
        map.insert(21, MaybeProtocol::Ftp);
        map.insert(22, MaybeProtocol::Ssh);
        map.insert(25, MaybeProtocol::Smtp);
        map.insert(80, MaybeProtocol::Http);
        map.insert(110, MaybeProtocol::Pop3);
        map.insert(119, MaybeProtocol::Nntp);
        map.insert(143, MaybeProtocol::Imap);
        map.insert(322, MaybeProtocol::Rtsps);
        map.insert(433, MaybeProtocol::Nntp);
        map.insert(443, MaybeProtocol::Https);
        map.insert(554, MaybeProtocol::Rtsp);
        map.insert(563, MaybeProtocol::Nntps);
        map.insert(587, MaybeProtocol::Smtp);
        map.insert(993, MaybeProtocol::Imaps);
        map.insert(995, MaybeProtocol::Pop3s);
        map.insert(1883, MaybeProtocol::Mqtt);
        map.insert(1935, MaybeProtocol::Rtmp);
        map.insert(4222, MaybeProtocol::Nats);
        map.insert(6881, MaybeProtocol::BitTorrent);
        map.insert(8080, MaybeProtocol::Http);
        map.insert(8883, MaybeProtocol::SecureMqtt);
        map
    }

    pub fn tcp_client() -> Self {
        let mut map = ProtocolPortMap::empty();
        map.insert(6881, MaybeProtocol::BitTorrent);
        map
    }

    #[inline]
    pub fn get(&self, port: u16) -> Option<&ProtocolPortMapValue> {
        self.inner.get(&port)
    }
}
