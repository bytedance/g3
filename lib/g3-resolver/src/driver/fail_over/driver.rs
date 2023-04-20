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

use std::time::Duration;

use tokio::sync::mpsc;

use super::FailOverDriverStaticConfig;
use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;
use crate::{
    ResolveDriver, ResolveJob, ResolveJobRecvResult, ResolveLocalError, ResolvedRecord,
    ResolverHandle,
};

pub(super) struct FailOverResolver {
    pub(super) primary: Option<ResolverHandle>,
    pub(super) standby: Option<ResolverHandle>,
    pub(super) conf: FailOverDriverStaticConfig,
}

struct FailOverResolverJob {
    primary: Option<ResolveJob>,
    standby: Option<ResolveJob>,
    job_timeout: Duration,
    config: FailOverDriverStaticConfig,
}

impl FailOverResolverJob {
    fn normalize_job_recv_result(
        &self,
        domain: &str,
        result: ResolveJobRecvResult,
    ) -> ResolvedRecord {
        match result {
            Ok((r, _)) => r.as_ref().clone(),
            Err(e) => {
                ResolvedRecord::failed(domain.to_string(), self.config.negative_ttl, e.into())
            }
        }
    }

    fn record_is_valid(&self, r: &ResolvedRecord) -> bool {
        if self.config.retry_empty_record {
            r.is_usable()
        } else {
            r.is_ok()
        }
    }

    async fn resolve(mut self, domain: &str) -> ResolvedRecord {
        let primary = self.primary.take();
        let standby = self.standby.take();
        match (primary, standby) {
            (Some(mut primary), Some(mut standby)) => {
                match tokio::time::timeout(self.config.fallback_delay, primary.recv()).await {
                    Ok(Ok((r, _))) => {
                        if self.record_is_valid(&r) {
                            r.as_ref().clone()
                        } else {
                            self.normalize_job_recv_result(domain, standby.recv().await)
                        }
                    }
                    Ok(Err(_)) => self.normalize_job_recv_result(domain, standby.recv().await),
                    Err(_) => {
                        // get the first success one, if not found, get the last error one
                        tokio::select! {
                            biased;

                            r = primary.recv() => {
                                match r {
                                    Ok((r, _)) => {
                                        if self.record_is_valid(&r) {
                                            r.as_ref().clone()
                                        } else {
                                            self.normalize_job_recv_result(domain, standby.recv().await)
                                        }
                                    }
                                    Err(_) => {
                                        self.normalize_job_recv_result(domain, standby.recv().await)
                                    }
                                }
                            }
                            r = standby.recv() => {
                                match r {
                                    Ok((r, _)) => {
                                        if self.record_is_valid(&r) {
                                            r.as_ref().clone()
                                        } else {
                                            self.normalize_job_recv_result(domain, primary.recv().await)
                                        }
                                    }
                                    Err(_) => {
                                        self.normalize_job_recv_result(domain, primary.recv().await)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            (Some(mut job), None) | (None, Some(mut job)) => {
                self.normalize_job_recv_result(domain, job.recv().await)
            }
            (None, None) => {
                self.normalize_job_recv_result(domain, Err(ResolveLocalError::NoResolverRunning))
            }
        }
    }

    async fn resolve_protective(self, domain: String) -> ResolvedRecord {
        let protective_cache_ttl = self.config.negative_ttl;
        tokio::time::timeout(self.job_timeout, self.resolve(&domain))
            .await
            .unwrap_or_else(|_| ResolvedRecord::timed_out(domain, protective_cache_ttl))
    }
}

impl ResolveDriver for FailOverResolver {
    fn query_v4(
        &self,
        domain: String,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    ) {
        let job_primary = self
            .primary
            .as_ref()
            .map(|handle| handle.get_v4(domain.clone()).map(Some).unwrap_or(None))
            .unwrap_or(None);
        let job_standby = self
            .standby
            .as_ref()
            .map(|handle| handle.get_v4(domain.clone()).map(Some).unwrap_or(None))
            .unwrap_or(None);
        let job = FailOverResolverJob {
            primary: job_primary,
            standby: job_standby,
            job_timeout: config.protective_query_timeout,
            config: self.conf,
        };
        tokio::spawn(async move {
            let record = job.resolve_protective(domain).await;
            let _ = sender.send(ResolveDriverResponse::V4(record)); // TODO log error
        });
    }

    fn query_v6(
        &self,
        domain: String,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    ) {
        let job_primary = self
            .primary
            .as_ref()
            .map(|handle| handle.get_v6(domain.clone()).map(Some).unwrap_or(None))
            .unwrap_or(None);
        let job_standby = self
            .standby
            .as_ref()
            .map(|handle| handle.get_v6(domain.clone()).map(Some).unwrap_or(None))
            .unwrap_or(None);
        let job = FailOverResolverJob {
            primary: job_primary,
            standby: job_standby,
            job_timeout: config.protective_query_timeout,
            config: self.conf,
        };
        tokio::spawn(async move {
            let record = job.resolve_protective(domain).await;
            let _ = sender.send(ResolveDriverResponse::V6(record)); // TODO log error
        });
    }
}
