/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2023-2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;

use g3_openssl::async_job::{OpensslAsyncOutput, SyncOperation, TokioAsyncOperation};

use super::KeylessOpensslArgs;

/// Generous bound for a single crypto op in the bench; matches keyless default
/// order of magnitude while avoiding false timeouts under load.
const ASYNC_OP_TIMEOUT: Duration = Duration::from_secs(30);

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
        let async_task = TokioAsyncOperation::build_async_task(self, ASYNC_OP_TIMEOUT)
            .map_err(|e| anyhow!("failed to create openssl async task: {e}"))?;
        match async_task.await {
            OpensslAsyncOutput::Finished(r) => r.map_err(anyhow::Error::new),
            OpensslAsyncOutput::TimedOut { cleanup } => {
                if let Some(cleanup) = cleanup {
                    let _ = cleanup.await;
                }
                Err(anyhow!("openssl async task timed out"))
            }
        }
    }
}
