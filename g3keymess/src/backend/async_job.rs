/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use openssl::pkey::{PKey, Private};
use tokio::sync::mpsc;

use g3_openssl::async_job::{SyncOperation, TokioAsyncOperation};

use super::{Backend, DispatchedKeylessRequest};
use crate::config::backend::AsyncJobBackendConfig;
use crate::protocol::{KeylessErrorResponse, KeylessResponse};
use crate::serve::{WrappedKeylessRequest, WrappedKeylessResponse};

pub(super) struct AsyncJobBackend {
    config: AsyncJobBackendConfig,
}

impl AsyncJobBackend {
    pub(super) fn new(config: AsyncJobBackendConfig) -> Self {
        AsyncJobBackend { config }
    }

    async fn loop_run(self, mut receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        while let Some(req) = receiver.recv().await {
            let DispatchedKeylessRequest {
                inner,
                key,
                rsp_sender,
            } = req;
            let req = inner;

            let req_server_stats = req.stats.clone();
            let crypto_fail = KeylessErrorResponse::new(req.inner.id).crypto_fail();
            let rsp = req.build_response(KeylessResponse::Error(crypto_fail));
            let sync_op = OpensslOperation::new(req, key);
            let Ok(task) = TokioAsyncOperation::build_async_task(sync_op) else {
                req_server_stats.add_crypto_fail();
                let _ = rsp_sender.send(rsp).await;
                continue;
            };

            let async_op_timeout = self.config.async_op_timeout;
            tokio::spawn(async move {
                let rsp = match tokio::time::timeout(async_op_timeout, task).await {
                    Ok(Ok(r)) => {
                        req_server_stats.add_passed();
                        r
                    }
                    Ok(Err(_)) => {
                        req_server_stats.add_crypto_fail();
                        rsp
                    }
                    Err(_) => {
                        req_server_stats.add_crypto_fail();
                        rsp
                    }
                };
                let _ = rsp_sender.send(rsp).await;
            });
        }
    }
}

impl Backend for AsyncJobBackend {
    async fn run_rsa_2048(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.loop_run(receiver).await
    }

    async fn run_rsa_3072(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.loop_run(receiver).await
    }

    async fn run_rsa_4096(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.loop_run(receiver).await
    }

    async fn run_ecdsa_p256(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.loop_run(receiver).await
    }

    async fn run_ecdsa_p384(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.loop_run(receiver).await
    }

    async fn run_ecdsa_p521(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.loop_run(receiver).await
    }
}

pub(crate) struct OpensslOperation {
    req: WrappedKeylessRequest,
    key: PKey<Private>,
}

impl OpensslOperation {
    pub(crate) fn new(req: WrappedKeylessRequest, key: PKey<Private>) -> Self {
        OpensslOperation { req, key }
    }
}

impl SyncOperation for OpensslOperation {
    type Output = WrappedKeylessResponse;

    fn run(&mut self) -> anyhow::Result<Self::Output> {
        let rsp = match self.req.inner.process(&self.key) {
            Ok(d) => KeylessResponse::Data(d),
            Err(e) => KeylessResponse::Error(e),
        };
        Ok(self.req.build_response(rsp))
    }
}
