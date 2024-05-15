/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::net::IpAddr;
use std::str;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use g3_io_ext::LineRecvBuf;
use g3_smtp_proto::command::Command;
use g3_smtp_proto::response::{ReplyCode, ResponseParser};
use g3_types::net::Host;

use super::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt};
use crate::serve::{ServerTaskError, ServerTaskResult};

pub struct Initiation {
    local_ip: IpAddr,
    client_host: Host,
}

impl Initiation {
    pub(super) fn new(local_ip: IpAddr) -> Self {
        Initiation {
            local_ip,
            client_host: Host::empty(),
        }
    }

    pub(super) fn into_parts(self) -> Host {
        self.client_host
    }

    pub(super) async fn relay<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut cmd_recv_buf = LineRecvBuf::<{ Command::MAX_LINE_SIZE }>::default();
        let mut rsp_recv_buf = LineRecvBuf::<{ ResponseParser::MAX_LINE_SIZE }>::default();

        loop {
            let Some(_cmd_line) = cmd_recv_buf
                .recv_cmd_and_relay(
                    clt_r,
                    clt_w,
                    ups_w,
                    |cmd| match cmd {
                        Command::ExtendHello(host) => {
                            self.client_host = host;
                            false
                        }
                        Command::Hello(host) => {
                            self.client_host = host;
                            false
                        }
                        _ => true,
                    },
                    self.local_ip,
                )
                .await?
            else {
                continue;
            };

            if self
                .relay_rsp(&mut rsp_recv_buf, ups_r, clt_w)
                .await?
                .is_some()
            {
                return Ok(());
            }
        }
    }

    async fn relay_rsp<CW, UR>(
        &mut self,
        rsp_recv_buf: &mut LineRecvBuf<{ ResponseParser::MAX_LINE_SIZE }>,
        ups_r: &mut UR,
        clt_w: &mut CW,
    ) -> ServerTaskResult<Option<()>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        let mut rsp = ResponseParser::default();
        loop {
            let line = rsp_recv_buf
                .read_rsp_line_with_feedback(ups_r, clt_w, self.local_ip)
                .await?;
            let msg = rsp
                .feed_line_with_feedback(line, clt_w, self.local_ip)
                .await?;

            if rsp.code() == ReplyCode::OK {
                if rsp.is_first_line() || self.allow_extension(msg) {
                    clt_w
                        .write_all(line)
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                }

                if rsp.finished() {
                    clt_w
                        .flush()
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                    return Ok(Some(()));
                }
            } else {
                clt_w
                    .write_all(line)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;

                if rsp.finished() {
                    clt_w
                        .flush()
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                    return Ok(None);
                }
            }
        }
    }

    fn allow_extension(&mut self, msg: &[u8]) -> bool {
        if let Some(p) = memchr::memchr(b' ', msg) {
            let Ok(keyword) = str::from_utf8(&msg[..p]) else {
                return false;
            };

            match keyword.to_uppercase().as_str() {
                "SIZE" => true, // Message Size Declaration, RFC1870, TODO use this max message limit ?
                "DELIVERBY" => true, // Deliver By, RFC2852, add a MAIL BY param key
                "NO-SOLICITING" => true, // No Soliciting, RFC3865, add a MAIL param key
                "AUTH" => true, // Authentication, RFC4954, add AUTH command
                "BURL" => true, // BURL, RFC4468, add BURL command
                "FUTURERELEASE" => true, // Future Message Release, RFC4865, add MAIL param keys
                "MT-PRIORITY" => true, // Priority Message Handling, RFC6710, add a MAIL param key
                "LIMITS" => true, // LIMITS, RFC9422
                _ => false,
            }
        } else {
            let Ok(keyword) = str::from_utf8(msg) else {
                return false;
            };

            match keyword.to_uppercase().as_str() {
                "EXPN" => true,                // Expand the mailing list, RFC5321, add EXPN command
                "HELP" => true, // Supply helpful information, RFC5321, add HELP command
                "8BITMIME" => true, // 8bit-MIMEtransport, RFC6152, add a MAIL BODY param value
                "SIZE" => true, // Message Size Declaration, RFC1870
                "VERB" => true, // Verbose
                "ONEX" => true, // One message transaction only
                "CHUNKING" => true, // CHUNKING, RFC3030, add BDAT command
                "BINARYMIME" => true, // BINARYMIME, RFC3030, add a MAIL BODY param value, require CHUNKING
                "DELIVERBY" => true,  // Deliver By, RFC2852, add a MAIL BY param key
                "PIPELINING" => true, // Pipelining, RFC2920
                "DSN" => true, // Delivery Status Notification, RFC3461, add param keys to RCPT and MAIL
                "ETRN" => true, // Remote Queue Processing Declaration, RFC1985, add ETRN command
                "ENHANCEDSTATUSCODES" => true, // Enhanced-Status-Codes, RFC2034, add status code preface to response
                "STARTTLS" => true,            // STARTTLS, RFC3207, add STARTTLS command
                "NO-SOLICITING" => true,       // No Soliciting, RFC3865, add a MAIL param key
                "MTRK" => true, // Message Tracking, RFC3885, add a MAIL MTRK param key
                "BURL" => true, // BURL, RFC4468, add BURL command, no param means AUTH is required
                "CONPERM" => true, // Content-Conversion-Permission, RFC4141, add a MAIL param key
                "CONNEG" => true, // Content-Negotiation, RFC4141, add a RCPT param key
                "SMTPUTF8" => true, // Internationalized Email, RFC6531, add MAIL/VRFY/EXPN param key
                "MT-PRIORITY" => true, // Priority Message Handling, RFC6710, add a MAIL param key
                "RRVS" => true,     // Require Recipient Valid Since, RFC7293, add a RCPT param key
                "REQUIRETLS" => true, // Require TLS, RFC8689, add a MAIL param key
                "LIMITS" => true,   // LIMITS, RFC9422
                "ATRN" => true,     // On-Demand Mail Relay, RFC2645, change the protocol
                _ => false,
            }
        }
    }
}
