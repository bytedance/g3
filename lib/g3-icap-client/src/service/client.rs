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

use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::oneshot;

use super::{
    IcapClientConnection, IcapConnectionCreator, IcapServiceClientCommand, IcapServiceConfig,
    IcapServicePool,
};
use crate::options::{IcapOptionsRequest, IcapServiceOptions};

pub struct IcapServiceClient {
    pub(crate) config: Arc<IcapServiceConfig>,
    pub(crate) partial_request_header: Vec<u8>,
    cmd_sender: flume::Sender<IcapServiceClientCommand>,
    conn_creator: Arc<IcapConnectionCreator>,
}

impl IcapServiceClient {
    pub fn new(config: Arc<IcapServiceConfig>) -> Self {
        let (cmd_sender, cmd_receiver) = flume::unbounded();
        let conn_creator = Arc::new(IcapConnectionCreator::new(config.clone()));
        let pool = IcapServicePool::new(config.clone(), cmd_receiver, conn_creator.clone());
        tokio::spawn(pool.into_running());
        let partial_request_header = config.build_request_header();
        IcapServiceClient {
            config,
            partial_request_header,
            cmd_sender,
            conn_creator,
        }
    }

    async fn fetch_from_pool(&self) -> Option<(IcapClientConnection, Arc<IcapServiceOptions>)> {
        let (rsp_sender, rsp_receiver) = oneshot::channel();
        let cmd = IcapServiceClientCommand::FetchConnection(rsp_sender);
        if self.cmd_sender.send_async(cmd).await.is_ok() {
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
        let options = options_req
            .get_options(&mut conn, self.config.icap_max_header_size)
            .await
            .map_err(|e| anyhow!("failed to get icap service options: {e}"))?;
        Ok((conn, Arc::new(options)))
    }

    pub async fn save_connection(&self, conn: IcapClientConnection) {
        let _ = self
            .cmd_sender
            .send_async(IcapServiceClientCommand::SaveConnection(conn))
            .await;
    }
}
