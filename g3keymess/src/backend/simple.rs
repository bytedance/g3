/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use tokio::sync::mpsc;

use super::{Backend, DispatchedKeylessRequest};

pub(super) struct SimpleBackend {}

impl SimpleBackend {
    pub(super) fn new() -> Self {
        SimpleBackend {}
    }

    async fn run(self, mut receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        while let Some(req) = receiver.recv().await {
            let rsp = req.inner.process_by_openssl(&req.key);
            let _ = req.rsp_sender.send(req.inner.build_response(rsp)).await;
        }
    }
}

impl Backend for SimpleBackend {
    async fn run_rsa_2048(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.run(receiver).await
    }

    async fn run_rsa_3072(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.run(receiver).await
    }

    async fn run_rsa_4096(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.run(receiver).await
    }

    async fn run_ecdsa_p256(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.run(receiver).await
    }

    async fn run_ecdsa_p384(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.run(receiver).await
    }

    async fn run_ecdsa_p521(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>) {
        self.run(receiver).await
    }
}
