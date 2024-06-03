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

use g3_dpi::SmtpInterceptionConfig;
use g3_io_ext::LineRecvBuf;
use g3_smtp_proto::command::Command;
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseParser};
use g3_types::net::Host;

use super::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt};
use crate::serve::{ServerTaskError, ServerTaskResult};

#[derive(Default)]
pub(super) struct InitializedExtensions {
    odmr: bool,
    starttls: bool,
    chunking: bool,
    burl: bool,
}

impl InitializedExtensions {
    pub(super) fn allow_odmr(&self, config: &SmtpInterceptionConfig) -> bool {
        self.odmr && config.allow_on_demand_mail_relay
    }

    pub(super) fn allow_starttls(&self, from_starttls: bool) -> bool {
        self.starttls && !from_starttls
    }

    pub(super) fn allow_chunking(&self) -> bool {
        self.chunking
    }

    pub(super) fn allow_burl(&self, config: &SmtpInterceptionConfig) -> bool {
        self.burl && config.allow_burl_data
    }
}

pub(super) struct Initiation<'a> {
    config: &'a SmtpInterceptionConfig,
    local_ip: IpAddr,
    from_starttls: bool,
    client_host: Host,
    server_ext: InitializedExtensions,
}

impl<'a> Initiation<'a> {
    pub(super) fn new(
        config: &'a SmtpInterceptionConfig,
        local_ip: IpAddr,
        from_starttls: bool,
    ) -> Self {
        Initiation {
            config,
            local_ip,
            from_starttls,
            client_host: Host::empty(),
            server_ext: InitializedExtensions::default(),
        }
    }

    pub(super) fn into_parts(self) -> (Host, InitializedExtensions) {
        (self.client_host, self.server_ext)
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
                            None
                        }
                        Command::Hello(host) => {
                            self.client_host = host;
                            None
                        }
                        _ => Some(ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS),
                    },
                    self.local_ip,
                )
                .await?
            else {
                continue;
            };

            if self
                .recv_relay_check_rsp(&mut rsp_recv_buf, ups_r, clt_w)
                .await?
                .is_some()
            {
                return Ok(());
            }
        }
    }

    async fn recv_relay_check_rsp<CW, UR>(
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
                // Message Size Declaration, RFC1870, TODO use this max message limit ?
                "SIZE" => true,
                // Deliver By, RFC2852, add a MAIL BY param key
                "DELIVERBY" => true,
                // No Soliciting, RFC3865, add a MAIL param key
                "NO-SOLICITING" => true,
                // Authentication, RFC4954, add AUTH command
                "AUTH" => true,
                // BURL, RFC4468, add BURL command
                "BURL" => {
                    self.server_ext.burl = true;
                    self.config.allow_burl_data
                }
                // Future Message Release, RFC4865, add MAIL param keys
                "FUTURERELEASE" => true,
                // Priority Message Handling, RFC6710, add a MAIL param key
                "MT-PRIORITY" => true,
                // LIMITS, RFC9422
                "LIMITS" => true,
                _ => false,
            }
        } else {
            let Ok(keyword) = str::from_utf8(msg) else {
                return false;
            };

            match keyword.to_uppercase().as_str() {
                // Expand the mailing list, RFC5321, add EXPN command
                "EXPN" => true,
                // Supply helpful information, RFC5321, add HELP command
                "HELP" => true,
                // 8bit-MIMEtransport, RFC6152, add a MAIL BODY param value
                "8BITMIME" => true,
                // Message Size Declaration, RFC1870
                "SIZE" => true,
                // Verbose
                "VERB" => true,
                // One message transaction only
                "ONEX" => true,
                // CHUNKING, RFC3030, add BDAT command
                "CHUNKING" => {
                    self.server_ext.chunking = true;
                    true
                }
                // BINARYMIME, RFC3030, add a MAIL BODY param value, require CHUNKING
                "BINARYMIME" => true,
                // Deliver By, RFC2852, add a MAIL BY param key
                "DELIVERBY" => true,
                // Pipelining, RFC2920
                "PIPELINING" => true,
                // Delivery Status Notification, RFC3461, add param keys to RCPT and MAIL
                "DSN" => true,
                // Remote Queue Processing Declaration, RFC1985, add ETRN command
                "ETRN" => true,
                // Enhanced-Status-Codes, RFC2034, add status code preface to response
                "ENHANCEDSTATUSCODES" => false,
                // STARTTLS, RFC3207, add STARTTLS command
                "STARTTLS" => {
                    self.server_ext.starttls = true;
                    !self.from_starttls
                }
                // No Soliciting, RFC3865, add a MAIL param key
                "NO-SOLICITING" => true,
                // Message Tracking, RFC3885, add a MAIL MTRK param key
                "MTRK" => true,
                // BURL, RFC4468, add BURL command, no param means AUTH is required
                "BURL" => {
                    self.server_ext.burl = true;
                    self.config.allow_burl_data
                }
                // Content-Conversion-Permission, RFC4141, add a MAIL param key
                "CONPERM" => true,
                // Content-Negotiation, RFC4141, add a RCPT param key
                "CONNEG" => true,
                // Internationalized Email, RFC6531, add MAIL/VRFY/EXPN param key
                "SMTPUTF8" => true,
                // Priority Message Handling, RFC6710, add a MAIL param key
                "MT-PRIORITY" => true,
                // Require Recipient Valid Since, RFC7293, add a RCPT param key
                "RRVS" => true,
                // Require TLS, RFC8689, add a MAIL param key
                "REQUIRETLS" => true,
                // LIMITS, RFC9422
                "LIMITS" => true,
                // On-Demand Mail Relay, RFC2645, change the protocol
                "ATRN" => {
                    self.server_ext.odmr = true;
                    self.config.allow_on_demand_mail_relay
                }
                _ => false,
            }
        }
    }
}
