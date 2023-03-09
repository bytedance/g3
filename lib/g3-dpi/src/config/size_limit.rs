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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolInspectionSizeLimit {
    pub(crate) ftp_server_greeting_msg: usize,
    pub(crate) http_client_request_uri: usize,
    pub(crate) imap_server_greeting_msg: usize,
    pub(crate) nats_server_info_line: usize,
    pub(crate) smtp_server_greeting_msg: usize,
}

impl Default for ProtocolInspectionSizeLimit {
    fn default() -> Self {
        ProtocolInspectionSizeLimit {
            ftp_server_greeting_msg: 512,
            http_client_request_uri: 4096,
            imap_server_greeting_msg: 512,
            nats_server_info_line: 1024,
            smtp_server_greeting_msg: 512,
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

    pub fn set_smtp_server_greeting_msg(&mut self, size: usize) {
        self.smtp_server_greeting_msg = size;
    }
}
