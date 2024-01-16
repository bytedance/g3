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

use std::collections::VecDeque;
use std::sync::Arc;

use fixedbitset::FixedBitSet;

use g3_types::net::AlpnProtocol;

use super::{MaybeProtocol, Protocol, ProtocolPortMap};
use crate::{ProtocolInspectionConfig, ProtocolInspectionSizeLimit};

const GUESS_PROTOCOL_FOR_CLIENT_INITIAL_DATA: &[MaybeProtocol] = &[
    MaybeProtocol::Ssl,
    MaybeProtocol::Http,
    MaybeProtocol::Ssh,
    MaybeProtocol::Smpp,
    MaybeProtocol::BitTorrent,
];
const GUESS_PROTOCOL_FOR_SERVER_INITIAL_DATA: &[MaybeProtocol] = &[
    MaybeProtocol::Ssh,
    MaybeProtocol::Ftp,
    MaybeProtocol::Nats,
    MaybeProtocol::BitTorrent,
];

#[derive(Debug)]
pub enum ProtocolInspectError {
    NeedMoreData(usize),
}

pub(crate) struct ProtocolInspectState {
    current: Option<MaybeProtocol>,
    excluded: FixedBitSet,
}

impl Default for ProtocolInspectState {
    fn default() -> Self {
        ProtocolInspectState {
            current: None,
            excluded: FixedBitSet::with_capacity(MaybeProtocol::_MaxSize as usize),
        }
    }
}

impl ProtocolInspectState {
    pub(crate) fn exclude_other(&mut self, protocol: MaybeProtocol) {
        self.excluded.insert(protocol as usize);
    }

    fn excluded(&self, protocol: MaybeProtocol) -> bool {
        self.excluded.contains(protocol as usize)
    }

    pub(crate) fn exclude_current(&mut self) {
        if let Some(p) = self.current.take() {
            self.excluded.insert(p as usize);
        }
    }

    #[inline]
    fn take_current(&mut self) -> Option<MaybeProtocol> {
        self.current.take()
    }

    #[inline]
    fn set_current(&mut self, protocol: MaybeProtocol) {
        self.current = Some(protocol);
    }

    fn reset_state(&mut self) {
        self.current = None;
        self.excluded.clear();
    }

    fn check_client_initial_data_for_protocol(
        &mut self,
        proto: MaybeProtocol,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        if self.excluded(proto) {
            return Ok(None);
        }
        self.current = Some(proto);
        match proto {
            MaybeProtocol::Ssh => self.check_ssh_client_protocol_version_exchange(data),
            MaybeProtocol::Dns => self.check_dns_tcp_request_message(data),
            MaybeProtocol::Http => self.check_http_request(data, size_limit),
            MaybeProtocol::Ssl => self.check_ssl_client_hello(data),
            MaybeProtocol::Rtsp => self.check_rtsp_client_setup_request(data),
            MaybeProtocol::Mqtt => self.check_mqtt_client_connect_request(data),
            MaybeProtocol::Stomp => self.check_stomp_client_connect_request(data),
            MaybeProtocol::Smpp => self.check_smpp_session_request(data),
            MaybeProtocol::Rtmp => self.check_rtmp_tcp_client_handshake(data),
            MaybeProtocol::BitTorrent => self.check_bittorrent_tcp_handshake(data),
            MaybeProtocol::Ftp
            | MaybeProtocol::Smtp
            | MaybeProtocol::Pop3
            | MaybeProtocol::Nntp
            | MaybeProtocol::Nnsp
            | MaybeProtocol::Imap
            | MaybeProtocol::Nats => {
                self.exclude_current();
                Ok(None)
            }
            MaybeProtocol::Https
            | MaybeProtocol::Pop3s
            | MaybeProtocol::Nntps
            | MaybeProtocol::Imaps
            | MaybeProtocol::Rtsps
            | MaybeProtocol::SecureMqtt
            | MaybeProtocol::Ssmpp
            | MaybeProtocol::Rtmps
            | MaybeProtocol::DnsOverTls
            | MaybeProtocol::_MaxSize => {
                unreachable!()
            }
        }
    }

    fn check_server_initial_data_for_protocol(
        &mut self,
        proto: MaybeProtocol,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        if self.excluded(proto) {
            return Ok(None);
        }
        self.current = Some(proto);
        match proto {
            MaybeProtocol::Ftp => self.check_ftp_server_greeting(data, size_limit),
            MaybeProtocol::Ssh => self.check_ssh_server_protocol_version_exchange(data),
            MaybeProtocol::Smtp => self.check_smtp_server_greeting(data, size_limit),
            MaybeProtocol::Pop3 => self.check_pop3_server_greeting(data),
            MaybeProtocol::Nntp | MaybeProtocol::Nnsp => self.check_nntp_server_greeting(data),
            MaybeProtocol::Imap => self.check_imap_server_greeting(data, size_limit),
            MaybeProtocol::Nats => self.check_nats_server_info_msg(data, size_limit),
            MaybeProtocol::BitTorrent => self.check_bittorrent_tcp_handshake(data),
            MaybeProtocol::Dns
            | MaybeProtocol::Ssl
            | MaybeProtocol::Http
            | MaybeProtocol::Rtsp
            | MaybeProtocol::Mqtt
            | MaybeProtocol::Stomp
            | MaybeProtocol::Smpp
            | MaybeProtocol::Rtmp => {
                self.exclude_current();
                Ok(None)
            }
            MaybeProtocol::Https
            | MaybeProtocol::Pop3s
            | MaybeProtocol::Nntps
            | MaybeProtocol::Imaps
            | MaybeProtocol::Rtsps
            | MaybeProtocol::SecureMqtt
            | MaybeProtocol::Ssmpp
            | MaybeProtocol::Rtmps
            | MaybeProtocol::DnsOverTls
            | MaybeProtocol::_MaxSize => {
                unreachable!()
            }
        }
    }
}

struct ReadPendingProtocol {
    size: usize,
    protocol: MaybeProtocol,
}

pub struct ProtocolInspector {
    server_portmap: Arc<ProtocolPortMap>,
    state: ProtocolInspectState,
    next_check_protocol: VecDeque<MaybeProtocol>,
    no_explicit_ssl: bool,
    read_pending_set: VecDeque<ReadPendingProtocol>,
    guess_protocols: bool,
}

impl Default for ProtocolInspector {
    fn default() -> Self {
        ProtocolInspector {
            server_portmap: Arc::new(ProtocolPortMap::tcp_server()),
            state: ProtocolInspectState::default(),
            next_check_protocol: VecDeque::with_capacity(4),
            no_explicit_ssl: false,
            read_pending_set: VecDeque::with_capacity(4),
            guess_protocols: true,
        }
    }
}

impl ProtocolInspector {
    pub fn new(
        server_portmap: Arc<ProtocolPortMap>,
        _client_portmap: Arc<ProtocolPortMap>,
    ) -> Self {
        ProtocolInspector {
            server_portmap,
            state: ProtocolInspectState::default(),
            next_check_protocol: VecDeque::with_capacity(4),
            no_explicit_ssl: false,
            read_pending_set: VecDeque::with_capacity(4),
            guess_protocols: true,
        }
    }

    pub fn push_protocol(&mut self, p: MaybeProtocol) {
        self.next_check_protocol.push_front(p);
    }

    pub fn push_alpn_protocol(&mut self, p: AlpnProtocol) {
        self.push_protocol(p.into());
    }

    pub fn reset_state(&mut self) {
        self.state.reset_state();
        self.guess_protocols = true;
    }

    pub fn set_no_explicit_ssl(&mut self) {
        self.no_explicit_ssl = true;
    }

    pub fn unset_no_explicit_ssl(&mut self) {
        self.no_explicit_ssl = false;
    }

    pub fn check_client_initial_data(
        &mut self,
        config: &ProtocolInspectionConfig,
        server_port: u16,
        data: &[u8],
    ) -> Result<Protocol, ProtocolInspectError> {
        macro_rules! check_protocol {
            ($p:expr) => {
                match self.state.check_client_initial_data_for_protocol(
                    $p,
                    data,
                    config.size_limit(),
                ) {
                    Ok(Some(p)) => return Ok(p),
                    Ok(None) => {}
                    Err(ProtocolInspectError::NeedMoreData(len)) => {
                        self.read_pending_set.push_back(ReadPendingProtocol {
                            size: len,
                            protocol: $p,
                        });
                    }
                }
            };
        }

        if let Some(proto) = self.state.take_current() {
            check_protocol!(proto);
        }

        while let Some(proto) = self.next_check_protocol.pop_front() {
            check_protocol!(proto);
        }

        if self.guess_protocols {
            if let Some(v) = self.server_portmap.get(server_port) {
                if !self.no_explicit_ssl && v.check_ssl() {
                    check_protocol!(MaybeProtocol::Ssl);
                }

                for proto in v.protocols() {
                    check_protocol!(*proto);
                }
            }

            for proto in GUESS_PROTOCOL_FOR_CLIENT_INITIAL_DATA {
                check_protocol!(*proto);
            }

            self.guess_protocols = false;
        }

        self.handle_read_pending()?;
        Ok(Protocol::Unknown)
    }

    pub fn check_server_initial_data(
        &mut self,
        config: &ProtocolInspectionConfig,
        server_port: u16,
        data: &[u8],
    ) -> Result<Protocol, ProtocolInspectError> {
        macro_rules! check_protocol {
            ($p:expr) => {
                match self.state.check_server_initial_data_for_protocol(
                    $p,
                    data,
                    config.size_limit(),
                ) {
                    Ok(Some(p)) => return Ok(p),
                    Ok(None) => {}
                    Err(ProtocolInspectError::NeedMoreData(len)) => {
                        self.read_pending_set.push_back(ReadPendingProtocol {
                            size: len,
                            protocol: $p,
                        });
                    }
                }
            };
        }

        if let Some(proto) = self.state.take_current() {
            check_protocol!(proto);
        }

        while let Some(proto) = self.next_check_protocol.pop_front() {
            check_protocol!(proto);
        }

        if self.guess_protocols {
            if let Some(v) = self.server_portmap.get(server_port) {
                for proto in v.protocols() {
                    check_protocol!(*proto);
                }
            }

            for proto in GUESS_PROTOCOL_FOR_SERVER_INITIAL_DATA {
                check_protocol!(*proto);
            }

            self.guess_protocols = false;
        }

        self.handle_read_pending()?;
        Ok(Protocol::Unknown)
    }

    fn handle_read_pending(&mut self) -> Result<(), ProtocolInspectError> {
        let Some(v) = self.read_pending_set.pop_front() else {
            return Ok(());
        };

        self.state.set_current(v.protocol);
        let mut pending_len = v.size;

        while let Some(v) = self.read_pending_set.pop_front() {
            if v.size < pending_len {
                pending_len = v.size;
            }
            self.next_check_protocol.push_front(v.protocol);
        }
        Err(ProtocolInspectError::NeedMoreData(pending_len))
    }
}
