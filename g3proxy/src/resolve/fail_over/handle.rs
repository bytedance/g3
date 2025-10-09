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
use g3_slog_types::LtDuration;
use g3_types::metrics::NodeName;

use crate::config::resolver::ResolverConfig;
use crate::config::resolver::fail_over::FailOverResolverConfig;
use crate::resolve::{BoxLoggedResolveJob, IntegratedResolverHandle, LoggedResolveJob};

pub(crate) struct FailOverResolverHandle {
    config: Arc<FailOverResolverConfig>,
    inner: g3_resolver::ResolverHandle,
    logger: Option<Logger>,
}

impl FailOverResolverHandle {
    pub(crate) fn new(
        config: &Arc<FailOverResolverConfig>,
        inner: g3_resolver::ResolverHandle,
        logger: Option<Logger>,
    ) -> Self {
        FailOverResolverHandle {
            config: Arc::clone(config),
            inner,
            logger,
        }
    }
}

impl IntegratedResolverHandle for FailOverResolverHandle {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn query_v4(&self, domain: Arc<str>) -> Result<BoxLoggedResolveJob, ResolveError> {
        let job = self.inner.get_v4(domain.clone())?;
        Ok(Box::new(FailOverResolverJob {
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
        Ok(Box::new(FailOverResolverJob {
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

struct FailOverResolverJob {
    config: Arc<FailOverResolverConfig>,
    domain: Arc<str>,
    query_type: ResolveQueryType,
    inner: g3_resolver::ResolveJob,
    logger: Option<Logger>,
    create_ins: Instant,
}

impl LoggedResolveJob for FailOverResolverJob {
    fn log_error(&self, e: &ResolveError, source: ResolvedRecordSource) {
        if let Some(logger) = &self.logger {
            slog::info!(logger, "{}", e;
                "next_primary" => &self.config.primary.as_str(),
                "next_standby" => &self.config.standby.as_str(),
                "query_type" => self.query_type.as_str(),
                "duration" => LtDuration(self.create_ins.elapsed()),
                "rr_source" => source.as_str(),
                "error_type" => e.get_type(),
                "error_subtype" => e.get_subtype(),
                "domain" => &self.domain,
            );
        }
    }

    impl_logged_poll_query!();
}
