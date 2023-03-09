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

use http::{Method, Uri};

use g3_http::server::HttpProxyClientRequest;
use g3_types::auth::{Password, Username};
use g3_types::net::{HttpAuth, HttpBasicAuth, UpstreamAddr};

use super::FtpRequestPath;
use crate::module::tcp_connect::TcpConnectTaskNotes;

pub(crate) struct FtpOverHttpTaskNotes {
    pub(crate) method: Method,
    pub(crate) uri: Uri,
    pub(crate) uri_log_max_chars: usize,
    pub(crate) rsp_status: u16,
    pub(crate) ftp_path: FtpRequestPath,
    ftp_user: Option<Username>,
    ftp_pass: Option<Password>,
    pub(crate) control_tcp_notes: TcpConnectTaskNotes,
    pub(crate) transfer_tcp_notes: TcpConnectTaskNotes,
}

impl FtpOverHttpTaskNotes {
    pub(crate) fn new(
        req: &HttpProxyClientRequest,
        upstream: &UpstreamAddr,
        uri_log_max_chars: usize,
    ) -> Self {
        let mut username: Option<Username> = None;
        let mut password: Option<Password> = None;

        if let Some(authority) = req.uri.authority() {
            let s = authority.as_str();

            if let Some(at_pos) = memchr::memchr(b'@', s.as_bytes()) {
                if let Some(p) = memchr::memchr(b':', s[0..at_pos].as_bytes()) {
                    username = Username::from_encoded(&s[0..p]).ok();
                    password = Password::from_encoded(&s[p + 1..at_pos]).ok();
                } else {
                    username = Username::from_encoded(&s[0..at_pos]).ok();
                }
            }
        }

        if let Some(v) = req.end_to_end_headers.get(http::header::AUTHORIZATION) {
            if let Ok(HttpAuth::Basic(HttpBasicAuth {
                username: u,
                password: p,
                ..
            })) = HttpAuth::try_from(v)
            {
                username = Some(u);
                password = Some(p);
            }
        }

        FtpOverHttpTaskNotes {
            method: req.method.clone(),
            uri: req.uri.clone(),
            uri_log_max_chars,
            rsp_status: 0,
            ftp_path: FtpRequestPath::from(&req.uri),
            ftp_user: username,
            ftp_pass: password,
            control_tcp_notes: TcpConnectTaskNotes::new(upstream.clone()),
            transfer_tcp_notes: TcpConnectTaskNotes::empty(),
        }
    }

    #[inline]
    pub(crate) fn username(&self) -> Option<&Username> {
        self.ftp_user.as_ref()
    }

    #[inline]
    pub(crate) fn password(&self) -> Option<&Password> {
        self.ftp_pass.as_ref()
    }

    #[inline]
    pub(crate) fn upstream(&self) -> &UpstreamAddr {
        &self.control_tcp_notes.upstream
    }
}
