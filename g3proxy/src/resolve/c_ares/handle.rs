/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::IpAddr;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use slog::{Logger, slog_info};
use tokio::time::Instant;

use g3_resolver::{ResolveError, ResolveQueryType, ResolvedRecordSource};
use g3_slog_types::{LtDuration, LtIpAddr};
use g3_types::metrics::NodeName;

use crate::config::resolver::ResolverConfig;
use crate::config::resolver::c_ares::CAresResolverConfig;
use crate::resolve::{BoxLoggedResolveJob, IntegratedResolverHandle, LoggedResolveJob};

pub(crate) struct CAresResolverHandle {
    config: Arc<CAresResolverConfig>,
    inner: g3_resolver::ResolverHandle,
    logger: Option<Logger>,
}

impl CAresResolverHandle {
    pub(crate) fn new(
        config: &Arc<CAresResolverConfig>,
        inner: g3_resolver::ResolverHandle,
        logger: Option<Logger>,
    ) -> Self {
        CAresResolverHandle {
            config: Arc::clone(config),
            inner,
            logger,
        }
    }
}

impl IntegratedResolverHandle for CAresResolverHandle {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn query_v4(&self, domain: Arc<str>) -> Result<BoxLoggedResolveJob, ResolveError> {
        let job = self.inner.get_v4(domain.clone())?;
        Ok(Box::new(CAresResolverJob {
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
        Ok(Box::new(CAresResolverJob {
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

struct CAresResolverJob {
    config: Arc<CAresResolverConfig>,
    domain: Arc<str>,
    query_type: ResolveQueryType,
    inner: g3_resolver::ResolveJob,
    logger: Option<Logger>,
    create_ins: Instant,
}

impl LoggedResolveJob for CAresResolverJob {
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
        slog_info!(logger, "{}", e;
            "bind_ipv4" => self.config.get_bind_ipv4().map(IpAddr::V4).map(LtIpAddr),
            "bind_ipv6" => self.config.get_bind_ipv6().map(IpAddr::V6).map(LtIpAddr),
            "server" => servers,
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
