/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::IpAddr;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use slog::Logger;
use tokio::time::Instant;

use g3_resolver::{ResolveError, ResolveQueryType, ResolvedRecordSource};
use g3_slog_types::{LtBindAddr, LtDuration};
use g3_types::metrics::NodeName;

use crate::config::resolver::ResolverConfig;
use crate::config::resolver::hickory::HickoryResolverConfig;
use crate::resolve::{BoxLoggedResolveJob, IntegratedResolverHandle, LoggedResolveJob};

pub(crate) struct HickoryResolverHandle {
    config: Arc<HickoryResolverConfig>,
    inner: g3_resolver::ResolverHandle,
    logger: Option<Logger>,
}

impl HickoryResolverHandle {
    pub(crate) fn new(
        config: &Arc<HickoryResolverConfig>,
        inner: g3_resolver::ResolverHandle,
        logger: Option<Logger>,
    ) -> Self {
        HickoryResolverHandle {
            config: Arc::clone(config),
            inner,
            logger,
        }
    }
}

impl IntegratedResolverHandle for HickoryResolverHandle {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn query_v4(&self, domain: Arc<str>) -> Result<BoxLoggedResolveJob, ResolveError> {
        let job = self.inner.get_v4(domain.clone())?;
        Ok(Box::new(HickoryResolverJob {
            config: Arc::clone(&self.config),
            domain,
            query_type: ResolveQueryType::A,
            inner: job,
            logger: self.logger.clone(),
            create_ins: Instant::now(),
        }))
    }

    fn query_v6(&self, domain: Arc<str>) -> Result<BoxLoggedResolveJob, ResolveError> {
        let job = self.inner.get_v6(domain.clone())?;
        Ok(Box::new(HickoryResolverJob {
            config: Arc::clone(&self.config),
            domain,
            query_type: ResolveQueryType::Aaaa,
            inner: job,
            logger: self.logger.clone(),
            create_ins: Instant::now(),
        }))
    }

    fn clone_inner(&self) -> Option<g3_resolver::ResolverHandle> {
        Some(self.inner.clone())
    }
}

struct HickoryResolverJob {
    config: Arc<HickoryResolverConfig>,
    domain: Arc<str>,
    query_type: ResolveQueryType,
    inner: g3_resolver::ResolveJob,
    logger: Option<Logger>,
    create_ins: Instant,
}

impl LoggedResolveJob for HickoryResolverJob {
    fn log_error(&self, e: &ResolveError, source: ResolvedRecordSource) {
        let Some(logger) = &self.logger else {
            return;
        };

        let servers = self
            .config
            .get_servers()
            .into_iter()
            .map(|server| server.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        slog::info!(logger, "{}", e; // TODO add encryption info
            "bind_addr" => LtBindAddr(self.config.get_bind_addr()),
            "server" => servers,
            "server_port" => self.config.get_server_port(),
            "encryption" => self.config.get_encryption_summary(),
            "query_type" => self.query_type.as_str(),
            "duration" => LtDuration(self.create_ins.elapsed()),
            "rr_source" => source.as_str(),
            "error_type" => e.get_type(),
            "error_subtype" => e.get_subtype(),
            "domain" => &self.domain,
        );
    }

    impl_logged_poll_query!();
}
