/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use super::FailOverDriverStaticConfig;
use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;
use crate::{
    ResolveDriver, ResolveJob, ResolveJobRecvResult, ResolveLocalError, ResolvedRecord,
    ResolvedRecordSource, ResolverHandle,
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
        domain: Arc<str>,
        result: ResolveJobRecvResult,
    ) -> ResolvedRecord {
        match result {
            Ok((r, _)) => r.as_ref().clone(),
            Err(e) => ResolvedRecord::failed(domain, self.config.negative_ttl, e.into()),
        }
    }

    fn record_is_valid(&self, r: &ResolvedRecord) -> bool {
        if self.config.retry_empty_record {
            r.is_usable()
        } else {
            r.is_ok()
        }
    }

    fn select_trash_usable(&self, r1: &ResolvedRecord, r2: ResolveJobRecvResult) -> ResolvedRecord {
        match r2 {
            Ok((_, ResolvedRecordSource::Trash)) => r1.clone(),
            Ok((r2, _)) => {
                if r2.is_usable() {
                    r2.as_ref().clone()
                } else {
                    r1.clone()
                }
            }
            Err(_) => r1.clone(),
        }
    }

    async fn resolve(mut self, domain: Arc<str>) -> ResolvedRecord {
        let primary = self.primary.take();
        let standby = self.standby.take();
        match (primary, standby) {
            (Some(mut primary), Some(mut standby)) => {
                match tokio::time::timeout(self.config.fallback_delay, primary.recv()).await {
                    Ok(Ok((r, ResolvedRecordSource::Trash))) => {
                        self.select_trash_usable(r.as_ref(), standby.recv().await)
                    }
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
                                    Ok((r, ResolvedRecordSource::Trash)) => {
                                        self.select_trash_usable(r.as_ref(), standby.recv().await)
                                    }
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
                                    Ok((r, ResolvedRecordSource::Trash)) => {
                                        self.select_trash_usable(r.as_ref(), primary.recv().await)
                                    }
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

    async fn resolve_protective(self, domain: Arc<str>) -> ResolvedRecord {
        let protective_cache_ttl = self.config.negative_ttl;
        tokio::time::timeout(self.job_timeout, self.resolve(domain.clone()))
            .await
            .unwrap_or_else(|_| ResolvedRecord::timed_out(domain, protective_cache_ttl))
    }
}

impl ResolveDriver for FailOverResolver {
    fn query_v4(
        &self,
        domain: Arc<str>,
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
        domain: Arc<str>,
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
