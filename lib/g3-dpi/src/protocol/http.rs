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

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};
use crate::ProtocolInspectionSizeLimit;

impl ProtocolInspectState {
    pub(crate) fn check_http_request(
        &mut self,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        macro_rules! check_method {
            ($m:literal) => {
                if data.starts_with($m) {
                    return self.check_http1_after_method(data, $m.len(), size_limit);
                }
            };
        }

        // at least <XYZ> <X> HTTP/<X>.<Y>\r\n\r\n
        const MINIMUM_DATA_LEN: usize = 18;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        match data[0] {
            b'A' => {
                // 0x41
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"ACL ");
            }
            b'B' => {
                // 0x42
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"BIND ");
                check_method!(b"BASELINE-CONTROL ");
            }
            b'C' => {
                // 0x43
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"CONNECT ");
                check_method!(b"COPY ");
                check_method!(b"CHECKIN ");
                check_method!(b"CHECKOUT ");
            }
            b'D' => {
                // 0x44
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"DELETE ");
            }
            b'G' => {
                // 0x47
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"GET ");
            }
            b'H' => {
                // 0x48
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"HEAD ");
            }
            b'L' => {
                // 0x4C
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"LOCK ");
                check_method!(b"LINK ");
                check_method!(b"LABEL ");
            }
            b'M' => {
                // 0x4D
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"MOVE ");
                check_method!(b"MKCOL ");
                check_method!(b"MERGE ");
                check_method!(b"MKACTIVITY ");
                check_method!(b"MKCALENDAR ");
                check_method!(b"MKREDIRECTREF ");
                check_method!(b"MKWORKSPACE ");
            }
            b'O' => {
                // 0x4F
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"OPTIONS ");
                check_method!(b"ORDERPATCH ");
            }
            b'P' => {
                // 0x50
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"POST ");
                check_method!(b"PUT ");
                if data.starts_with(b"PRI ") {
                    return self.check_http2_connection_preface(data);
                }
                check_method!(b"PROPFIND ");
                check_method!(b"PROPPATCH ");
                check_method!(b"PATCH ");
            }
            b'R' => {
                // 0x52
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"REPORT ");
                check_method!(b"REBIND ");
            }
            b'S' => {
                // 0x53
                self.http_exclude_by_byte0();

                if data.starts_with(b"SEARCH ") {
                    self.exclude_other(MaybeProtocol::Ssh);
                    self.exclude_other(MaybeProtocol::Rtsp);
                    return self.check_http1_after_method(data, 7, size_limit);
                }
                if data.starts_with(b"SOURCE ") {
                    // Icecast, deprecated since 2.4.0
                    self.exclude_other(MaybeProtocol::Ssh);
                    self.exclude_other(MaybeProtocol::Rtsp);
                    return self.check_http1_after_method(data, 7, size_limit);
                }
            }
            b'T' => {
                // 0x54
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"TRACE ");
            }
            b'U' => {
                // 0x55
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"UNLOCK ");
                check_method!(b"UNBIND ");
                check_method!(b"UNCHECKOUT ");
                check_method!(b"UNLINK ");
                check_method!(b"UPDATE ");
                check_method!(b"UNDATEREDIRECTREF ");
            }
            b'V' => {
                // 0x56
                self.http_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ssh);

                check_method!(b"VERSION-CONTROL ");
            }
            _ => {}
        }

        self.exclude_current();
        Ok(None)
    }

    fn http_exclude_by_byte0(&mut self) {
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);
    }

    fn check_http1_after_method(
        &mut self,
        data: &[u8],
        url_offset: usize,
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least <X> HTTP/<X>.<Y>\r\n\r\n
        const MINIMUM_DATA_LEN: usize = 14;

        let minimum_len = MINIMUM_DATA_LEN + url_offset;
        let data_len = data.len();
        if data_len < minimum_len {
            return Err(ProtocolInspectError::NeedMoreData(minimum_len - data_len));
        }

        let _method = &data[0..url_offset - 1];
        let left = &data[url_offset..];
        if let Some(p) = memchr::memchr(b' ', left) {
            // FIXME validate url?
            self.check_http1_after_url(data, url_offset + p + 1)
        } else if left.len() > size_limit.http_client_request_uri {
            self.exclude_current();
            Ok(None)
        } else {
            Err(ProtocolInspectError::NeedMoreData(1))
        }
    }

    fn check_http1_after_url(
        &mut self,
        data: &[u8],
        version_offset: usize,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least HTTP/<X>.<Y>\r\n\r\n
        const MINIMUM_DATA_LEN: usize = 12;

        let minimum_len = MINIMUM_DATA_LEN + version_offset;
        let data_len = data.len();
        if data_len < minimum_len {
            return Err(ProtocolInspectError::NeedMoreData(minimum_len - data_len));
        }

        let left = &data[version_offset..];
        if left.starts_with(b"HTTP/1.")
            && matches!(left[7], b'0' | b'1')
            && left[8] == b'\r'
            && left[9] == b'\n'
        {
            return Ok(Some(Protocol::Http1));
        }

        self.exclude_current();
        Ok(None)
    }

    fn check_http2_connection_preface(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        const H2_CONNECTION_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        const H2_CONNECTION_PREFACE_LEN: usize = H2_CONNECTION_PREFACE.len();

        let data_len = data.len();
        if data_len < H2_CONNECTION_PREFACE_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                H2_CONNECTION_PREFACE_LEN - data_len,
            ));
        }

        if !data.starts_with(H2_CONNECTION_PREFACE) {
            return Ok(None);
        }

        Ok(Some(Protocol::Http2))
    }
}
