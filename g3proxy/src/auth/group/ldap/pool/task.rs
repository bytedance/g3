/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::sync::Arc;

use anyhow::anyhow;
use kanal::AsyncReceiver;
use log::{debug, warn};
use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::openssl::MaybeSslStream;
use g3_io_ext::{AsyncStream, LimitedWriteExt};
use g3_types::codec::{LdapResult, LdapSequence};

use super::{LdapAuthRequest, LdapConnector};
use crate::auth::group::ldap::{LdapMessageReceiver, SimpleBindRequestEncoder};
use crate::config::auth::LdapUserGroupConfig;

pub(super) struct LdapAuthTask {
    config: Arc<LdapUserGroupConfig>,
    connector: Arc<LdapConnector>,
    quit: bool,
    pending_request: Option<LdapAuthRequest>,
    request_encoder: SimpleBindRequestEncoder,
}

impl LdapAuthTask {
    pub(super) fn new(config: Arc<LdapUserGroupConfig>, connector: Arc<LdapConnector>) -> Self {
        LdapAuthTask {
            config,
            connector,
            quit: false,
            pending_request: None,
            request_encoder: SimpleBindRequestEncoder::default(),
        }
    }

    pub(super) async fn run(
        mut self,
        receiver: AsyncReceiver<LdapAuthRequest>,
    ) -> anyhow::Result<()> {
        loop {
            let r = match self.connector.connect().await? {
                MaybeSslStream::Plain(stream) => self.run_with_stream(stream, &receiver).await,
                MaybeSslStream::Ssl(stream) => self.run_with_stream(stream, &receiver).await,
            };
            if let Err(e) = r {
                warn!("connection closed with error: {e}");
            }

            if let Some(mut request) = self.pending_request
                && request.retry
            {
                request.retry = false;
                self.pending_request = Some(request);
                continue;
            }

            return Ok(());
        }
    }

    async fn run_with_stream<S>(
        &mut self,
        stream: S,
        req_receiver: &AsyncReceiver<LdapAuthRequest>,
    ) -> anyhow::Result<()>
    where
        S: AsyncStream,
        S::R: AsyncRead + Unpin,
        S::W: AsyncWrite + Unpin,
    {
        self.request_encoder.reset();
        let (mut reader, mut writer) = stream.into_split();
        let mut ldap_rsp_receiver = LdapMessageReceiver::new(self.config.max_message_size);

        loop {
            if let Some(r) = self.pending_request.take() {
                let message_id = match self.send_simple_bind_request(&mut writer, &r).await {
                    Ok(id) => id,
                    Err(e) => {
                        self.pending_request = Some(r);
                        return Err(anyhow!("send simple bind request error: {e}"));
                    }
                };

                match tokio::time::timeout(
                    self.config.response_timeout,
                    ldap_rsp_receiver.recv(&mut reader),
                )
                .await
                {
                    Ok(Ok(message)) => {
                        if message.id() == 0 {
                            self.pending_request = Some(r);
                            let reconnect = self
                                .handle_unsolicited_notification(message.payload())
                                .map_err(|e| anyhow!("invalid unsolicited notification: {e}"))?;
                            if reconnect {
                                return Ok(());
                            } else {
                                continue;
                            }
                        } else if message.id() != message_id {
                            self.pending_request = Some(r);
                            debug!("unexpected response for message {}", message.id());
                            continue;
                        } else {
                            self.handle_response(message.payload(), r)
                                .map_err(|e| anyhow!("invalid response: {e}"))?;
                        }
                    }
                    Ok(Err(e)) => {
                        self.pending_request = Some(r);
                        return Err(anyhow!("recv ldap response message error: {e}"));
                    }
                    Err(_) => {
                        let _ = r.result_sender.send(None);
                        return Err(anyhow!("recv ldap response message timed out"));
                    }
                }
            }

            let timeout = tokio::time::sleep(self.config.connection_pool.idle_timeout());
            tokio::select! {
                biased;

                r = req_receiver.recv() => {
                    match r {
                        Ok(r) => self.pending_request = Some(r),
                        Err(_) => {
                            self.quit = true;
                            return Ok(());
                        }
                    }
                }
                _ = timeout => {
                    return self.send_unbind(&mut writer).await;
                }
                r = ldap_rsp_receiver.recv(&mut reader) => {
                    // detect the close of ldap server
                    match r {
                        Ok(message) => {
                            if message.id() != 0 {
                                debug!("unexpected response received for message {}", message.id());
                            } else {
                                let reconnect = self
                                    .handle_unsolicited_notification(message.payload())
                                    .map_err(|e| anyhow!("invalid unsolicited notification: {e}"))?;
                                if reconnect {
                                    return Ok(());
                                } else {
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            return Err(anyhow!("ldap connection closed with error {e}"));
                        }
                    }
                }
            }
        }
    }

    async fn send_simple_bind_request<W>(
        &mut self,
        writer: &mut W,
        r: &LdapAuthRequest,
    ) -> anyhow::Result<u32>
    where
        W: AsyncWrite + Unpin,
    {
        let bind_dn = format!(
            "{}={},{}",
            self.config.username_attribute, r.username, self.config.base_dn
        );
        let request_msg = self.request_encoder.encode(&bind_dn, &r.password);
        writer
            .write_all_flush(request_msg)
            .await
            .map_err(|e| anyhow!("failed to write bind request: {e}"))?;
        Ok(self.request_encoder.message_id())
    }

    async fn send_unbind<W>(&mut self, writer: &mut W) -> anyhow::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let unbind_message = self.request_encoder.unbind_sequence();
        writer
            .write_all_flush(&unbind_message)
            .await
            .map_err(|e| anyhow!("failed to write unbind request: {e}"))
    }

    fn handle_unsolicited_notification(&self, op_data: &[u8]) -> anyhow::Result<bool> {
        let rsp_sequence = LdapSequence::parse_extended_response(op_data)?;
        let data = rsp_sequence.data();
        let result = LdapResult::parse(data)?;
        let left = &data[result.encoded_len()..];
        let oid = LdapSequence::parse_extended_response_oid(left)?;
        if oid.data() == b"1.3.6.1.4.1.1466.20036" {
            // The notice of disconnection unsolicited notification OID
            Ok(true)
        } else {
            // TODO log other OID
            Ok(false)
        }
    }

    fn handle_response(&self, op_data: &[u8], r: LdapAuthRequest) -> anyhow::Result<()> {
        let rsp_sequence = LdapSequence::parse_bind_response(op_data)?;
        let data = rsp_sequence.data();
        let result = LdapResult::parse(data)?;
        if result.result_code() == 0 {
            let _ = r.result_sender.send(Some((r.username, r.password)));
        } else {
            // TODO log error
            let _ = r.result_sender.send(None);
        }
        Ok(())
    }
}
