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

    pub(super) async fn run(mut self, receiver: AsyncReceiver<LdapAuthRequest>) {
        loop {
            if self.quit {
                return;
            }

            let r = match self.connector.connect().await {
                Ok(MaybeSslStream::Plain(stream)) => self.run_with_stream(stream, &receiver).await,
                Ok(MaybeSslStream::Ssl(stream)) => self.run_with_stream(stream, &receiver).await,
                Err(e) => Err(anyhow!("failed to connect to ldap server: {e}")),
            };
            if let Err(e) = r {
                warn!("close connection on error: {e}");
                // TODO sleep some time
            }
        }
    }

    async fn run_with_stream<S>(
        &mut self,
        stream: S,
        receiver: &AsyncReceiver<LdapAuthRequest>,
    ) -> anyhow::Result<()>
    where
        S: AsyncStream,
        S::R: AsyncRead + Unpin,
        S::W: AsyncWrite + Unpin,
    {
        self.request_encoder.reset();
        let (mut reader, mut writer) = stream.into_split();
        let mut response_receiver = LdapMessageReceiver::new(self.config.max_message_size);

        loop {
            if let Some(r) = self.pending_request.take() {
                let message_id = match self.send_simple_bind_request(&mut writer, &r).await {
                    Ok(id) => id,
                    Err(e) => {
                        self.pending_request = Some(r);
                        return Err(anyhow!("send simple bind request error: {e}"));
                    }
                };

                // TODO timeout
                match response_receiver.recv(&mut reader).await {
                    Ok(message) => {
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
                    Err(e) => {
                        self.pending_request = Some(r);
                        return Err(anyhow!("recv ldap response message error: {e}"));
                    }
                }
            }

            match receiver.recv().await {
                Ok(r) => self.pending_request = Some(r),
                Err(_) => {
                    self.quit = true;
                    return Ok(());
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
        let request_msg = self
            .request_encoder
            .encode(&r.uid, &r.password, &self.config.base_dn);
        writer
            .write_all_flush(request_msg)
            .await
            .map_err(|e| anyhow!("failed to write bind request: {e}"))?;
        Ok(self.request_encoder.message_id())
    }

    fn handle_unsolicited_notification(&self, op_data: &[u8]) -> anyhow::Result<bool> {
        // TODO

        Ok(true)
    }

    fn handle_response(&self, op_data: &[u8], r: LdapAuthRequest) -> anyhow::Result<()> {
        todo!()
    }
}
