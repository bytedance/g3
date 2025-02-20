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

use std::net::IpAddr;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use slog::{Logger, slog_info};
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
    logger: Arc<Logger>,
}

impl FailOverResolverHandle {
    pub(crate) fn new(
        config: &Arc<FailOverResolverConfig>,
        inner: g3_resolver::ResolverHandle,
        logger: &Arc<Logger>,
    ) -> Self {
        FailOverResolverHandle {
            config: Arc::clone(config),
            inner,
            logger: Arc::clone(logger),
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
            logger: Arc::clone(&self.logger),
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
            logger: Arc::clone(&self.logger),
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
    logger: Arc<Logger>,
    create_ins: Instant,
}

impl LoggedResolveJob for FailOverResolverJob {
    fn log_error(&self, e: &ResolveError, source: ResolvedRecordSource) {
        slog_info!(&self.logger, "{}", e;
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

    impl_logged_poll_query!();
}
