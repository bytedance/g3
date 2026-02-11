/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use g3_codec::ldap::{LdapLength, LdapLengthParseError, LdapMessageId};

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};
use crate::ProtocolInspectionSizeLimit;

impl ProtocolInspectState {
    pub(crate) fn check_ldap_tcp_request_message(
        &mut self,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // 0x30 <LEN> 0x02 <Byte count of ID> <ID>
        const MINIMUM_DATA_LEN: usize = 5;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        if data[0] != 0x30 {
            self.exclude_current();
            return Ok(None);
        }

        self.exclude_other(MaybeProtocol::BitTorrent);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Smpp);
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Stomp);

        let header_len;
        let content_len;
        match LdapLength::parse(&data[1..]) {
            Ok(v) => {
                if v.value() > size_limit.ldap_request_msg as u64 {
                    self.exclude_current();
                    return Ok(None);
                }
                header_len = v.encoded_len() + 1;
                content_len = v.value() as usize;
                if data.len() < header_len + content_len {
                    return Err(ProtocolInspectError::NeedMoreData(
                        header_len + content_len - data.len(),
                    ));
                }
            }
            Err(LdapLengthParseError::NeedMoreData(n)) => {
                return Err(ProtocolInspectError::NeedMoreData(n));
            }
            Err(_) => {
                self.exclude_current();
                return Ok(None);
            }
        }

        let content = &data[header_len..header_len + content_len];
        match LdapMessageId::parse(content) {
            Ok(id) => {
                if id.value() == 0 {
                    self.exclude_current();
                    return Ok(None);
                }
            }
            Err(_) => {
                self.exclude_current();
                return Ok(None);
            }
        }

        // TODO check protocolOP

        Ok(Some(Protocol::Ldap))
    }
}
