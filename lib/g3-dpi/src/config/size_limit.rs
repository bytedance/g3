/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolInspectionSizeLimit {
    pub(crate) ftp_server_greeting_msg: usize,
    pub(crate) http_client_request_uri: usize,
    pub(crate) imap_server_greeting_msg: usize,
    pub(crate) nats_server_info_line: usize,
    pub(crate) ldap_request_msg: usize,
}

impl Default for ProtocolInspectionSizeLimit {
    fn default() -> Self {
        ProtocolInspectionSizeLimit {
            ftp_server_greeting_msg: 512,
            http_client_request_uri: 4096,
            imap_server_greeting_msg: 512,
            nats_server_info_line: 1024,
            ldap_request_msg: 1024,
        }
    }
}

impl ProtocolInspectionSizeLimit {
    pub fn set_ftp_server_greeting_msg(&mut self, size: usize) {
        self.ftp_server_greeting_msg = size;
    }

    pub fn set_http_client_request_uri(&mut self, size: usize) {
        self.http_client_request_uri = size;
    }

    pub fn set_imap_server_greeting_msg(&mut self, size: usize) {
        self.imap_server_greeting_msg = size;
    }

    pub fn set_nats_server_info_line(&mut self, size: usize) {
        self.nats_server_info_line = size;
    }

    pub fn set_ldap_request_msg(&mut self, size: usize) {
        self.ldap_request_msg = size;
    }
}
