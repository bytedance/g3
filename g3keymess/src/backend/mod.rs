/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use openssl::pkey::{PKey, Private};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::config::backend::BackendDriverConfig;
use crate::serve::{WrappedKeylessRequest, WrappedKeylessResponse};

mod dispatch;
pub(crate) use dispatch::dispatch;

#[cfg(feature = "openssl-async-job")]
mod async_job;
#[cfg(feature = "openssl-async-job")]
pub(crate) use async_job::OpensslOperation;

mod simple;

pub(crate) struct DispatchedKeylessRequest {
    pub(crate) inner: WrappedKeylessRequest,
    pub(crate) key: PKey<Private>,
    pub(crate) rsp_sender: mpsc::Sender<WrappedKeylessResponse>,
}

trait Backend {
    async fn run_rsa_2048(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>);
    async fn run_rsa_3072(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>);
    async fn run_rsa_4096(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>);
    async fn run_ecdsa_p256(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>);
    async fn run_ecdsa_p384(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>);
    async fn run_ecdsa_p521(self, receiver: mpsc::Receiver<DispatchedKeylessRequest>);
}

pub fn create(_id: usize, handle: &Handle) -> anyhow::Result<()> {
    let config = crate::config::backend::get_config();

    macro_rules! setup {
        ($run:ident, $register:ident) => {
            let (sender, receiver) = mpsc::channel(config.dispatch_channel_size);
            match config.driver {
                BackendDriverConfig::Simple => {
                    let backend = simple::SimpleBackend::new();
                    handle.spawn(backend.$run(receiver));
                }
                #[cfg(feature = "openssl-async-job")]
                BackendDriverConfig::AsyncJob(config) => {
                    let backend = async_job::AsyncJobBackend::new(config);
                    handle.spawn(backend.$run(receiver));
                }
            }
            dispatch::$register(sender, config.dispatch_counter_shift);
        };
    }

    setup!(run_rsa_2048, register_rsa_2048);
    setup!(run_rsa_3072, register_rsa_3072);
    setup!(run_rsa_4096, register_rsa_4096);
    setup!(run_ecdsa_p256, register_ecdsa_p256);
    setup!(run_ecdsa_p384, register_ecdsa_p384);
    setup!(run_ecdsa_p521, register_ecdsa_p521);

    Ok(())
}
