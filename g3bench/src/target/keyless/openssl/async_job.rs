/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;

use g3_openssl::async_job::{SyncOperation, TokioAsyncOperation};

use super::KeylessOpensslArgs;

pub(super) struct KeylessOpensslAsyncJob {
    args: Arc<KeylessOpensslArgs>,
}

impl SyncOperation for KeylessOpensslAsyncJob {
    type Output = Vec<u8>;

    fn run(&mut self) -> anyhow::Result<Self::Output> {
        self.args.handle_action()
    }
}

impl KeylessOpensslAsyncJob {
    pub(super) fn new(args: Arc<KeylessOpensslArgs>) -> Self {
        KeylessOpensslAsyncJob { args }
    }

    pub(super) async fn run(self) -> anyhow::Result<Vec<u8>> {
        let async_task = TokioAsyncOperation::build_async_task(self)
            .map_err(|e| anyhow!("failed to create openssl async task: {e}"))?;
        async_task.await.map_err(anyhow::Error::new)
    }
}
