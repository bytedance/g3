/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;

pub mod fail_over;

#[cfg(feature = "c-ares")]
pub mod c_ares;

#[cfg(feature = "hickory")]
pub mod hickory;

#[derive(Clone, Debug, PartialEq)]
pub enum AnyResolveDriverConfig {
    FailOver(fail_over::FailOverDriverConfig),
    #[cfg(feature = "c-ares")]
    CAres(c_ares::CAresDriverConfig),
    #[cfg(feature = "hickory")]
    Hickory(Box<hickory::HickoryDriverConfig>),
}

impl AnyResolveDriverConfig {
    pub(crate) fn spawn_resolver_driver(&self) -> anyhow::Result<Box<dyn ResolveDriver>> {
        match self {
            AnyResolveDriverConfig::FailOver(c) => Ok(c.spawn_resolver_driver()),
            #[cfg(feature = "c-ares")]
            AnyResolveDriverConfig::CAres(c) => c.spawn_resolver_driver(),
            #[cfg(feature = "hickory")]
            AnyResolveDriverConfig::Hickory(c) => c.spawn_resolver_driver(),
        }
    }
}

pub(crate) trait ResolveDriver {
    fn query_v4(
        &self,
        domain: Arc<str>,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    );
    fn query_v6(
        &self,
        domain: Arc<str>,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    );
}

pub(crate) type BoxResolverDriver = Box<dyn ResolveDriver>;
