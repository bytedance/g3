/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::oneshot;

use super::{
    IcapClientConnection, IcapConnector, IcapServiceClientCommand, IcapServiceConfig,
    IcapServicePool,
};
use crate::options::{IcapOptionsRequest, IcapServiceOptions};

pub struct IcapServiceClient {
    pub(crate) config: Arc<IcapServiceConfig>,
    pub(crate) partial_request_header: Vec<u8>,
    cmd_sender: kanal::AsyncSender<IcapServiceClientCommand>,
    conn_creator: Arc<IcapConnector>,
}

impl IcapServiceClient {
    pub fn new(config: Arc<IcapServiceConfig>) -> anyhow::Result<Self> {
        let (cmd_sender, cmd_receiver) = kanal::unbounded_async();
        let conn_creator = IcapConnector::new(config.clone())?;
        let conn_creator = Arc::new(conn_creator);
        let pool = IcapServicePool::new(config.clone(), cmd_receiver, conn_creator.clone());
        tokio::spawn(pool.into_running());
        let partial_request_header = config.build_request_header();
        Ok(IcapServiceClient {
            config,
            partial_request_header,
            cmd_sender,
            conn_creator,
        })
    }

    async fn fetch_from_pool(&self) -> Option<(IcapClientConnection, Arc<IcapServiceOptions>)> {
        let (rsp_sender, rsp_receiver) = oneshot::channel();
        let cmd = IcapServiceClientCommand::FetchConnection(rsp_sender);
        if self.cmd_sender.send(cmd).await.is_ok() {
            rsp_receiver.await.ok()
        } else {
            None
        }
    }

    pub async fn fetch_connection(
        &self,
    ) -> anyhow::Result<(IcapClientConnection, Arc<IcapServiceOptions>)> {
        if let Some(conn) = self.fetch_from_pool().await {
            return Ok(conn);
        }

        let mut conn = self
            .conn_creator
            .create()
            .await
            .map_err(|e| anyhow!("create new connection failed: {e:?}"))?;
        let options_req = IcapOptionsRequest::new(self.config.as_ref());

        conn.mark_io_inuse();
        let options = options_req
            .get_options(&mut conn, self.config.icap_max_header_size)
            .await
            .map_err(|e| anyhow!("failed to get icap service options: {e}"))?;

        conn.mark_io_inuse();
        Ok((conn, Arc::new(options)))
    }

    pub fn save_connection(&self, conn: IcapClientConnection) {
        if conn.reusable() {
            let pool_sender = self.cmd_sender.clone();
            tokio::spawn(async move {
                let _ = pool_sender
                    .send(IcapServiceClientCommand::SaveConnection(conn))
                    .await;
            });
        }
    }
}
