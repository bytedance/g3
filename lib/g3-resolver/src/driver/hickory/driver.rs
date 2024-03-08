/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;

use super::DnsRequest;
use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;
use crate::{ResolveDriver, ResolveDriverError, ResolveLocalError, ResolvedRecord};

#[derive(Clone)]
pub struct HickoryResolver {
    each_timeout: Duration,
    retry_interval: Duration,
    negative_min_ttl: u32,
    clients: Vec<flume::Sender<(DnsRequest, mpsc::Sender<ResolvedRecord>)>>,
}

impl ResolveDriver for HickoryResolver {
    fn query_v4(
        &self,
        domain: Arc<str>,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    ) {
        let request = DnsRequest::query_ipv4(domain.clone());

        let job = self.clone();
        let timeout = config.protective_query_timeout;
        tokio::spawn(async move {
            let r = run_timed(job, timeout, domain, request).await;
            let _ = sender.send(ResolveDriverResponse::V4(r));
        });
    }

    fn query_v6(
        &self,
        domain: Arc<str>,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    ) {
        let request = DnsRequest::query_ipv6(domain.clone());

        let job = self.clone();
        let timeout = config.protective_query_timeout;
        tokio::spawn(async move {
            let r = run_timed(job, timeout, domain, request).await;
            let _ = sender.send(ResolveDriverResponse::V6(r));
        });
    }
}

async fn run_timed(
    job: HickoryResolver,
    timeout: Duration,
    domain: Arc<str>,
    request: DnsRequest,
) -> ResolvedRecord {
    let error_ttl = job.negative_min_ttl;
    match tokio::time::timeout(timeout, job.run(domain.clone(), request)).await {
        Ok(r) => r,
        Err(_) => ResolvedRecord::timed_out(domain, error_ttl),
    }
}

impl HickoryResolver {
    pub(super) fn new(
        each_timeout: Duration,
        retry_interval: Duration,
        negative_min_ttl: u32,
    ) -> Self {
        HickoryResolver {
            each_timeout,
            retry_interval,
            negative_min_ttl,
            clients: Vec::with_capacity(2),
        }
    }

    pub(super) fn push_client(
        &mut self,
        req_sender: flume::Sender<(DnsRequest, mpsc::Sender<ResolvedRecord>)>,
    ) {
        self.clients.push(req_sender);
    }

    async fn run(self, domain: Arc<str>, request: DnsRequest) -> ResolvedRecord {
        let (rsp_sender, mut rsp_receiver) = mpsc::channel::<ResolvedRecord>(1);

        let mut wait_left = self.clients.len();
        let mut clients = self.clients.into_iter();
        let Some(client) = clients.next() else {
            return ResolvedRecord::failed(
                domain,
                self.negative_min_ttl,
                ResolveLocalError::NoResolverRunning.into(),
            );
        };
        if client
            .send_async((request.clone(), rsp_sender.clone()))
            .await
            .is_err()
        {
            wait_left -= 1;
        }

        let mut last_err: Option<ResolvedRecord> = None;
        let mut interval =
            tokio::time::interval_at(Instant::now() + self.retry_interval, self.retry_interval);
        loop {
            tokio::select! {
                biased;

                r = rsp_receiver.recv() => {
                    wait_left -= 1;
                    match r {
                        Some(v) => {
                            if v.is_ok() || wait_left == 0 {
                                return v;
                            }
                            last_err = Some(v);
                        }
                        None => unreachable!(), // as we keep a rsp_sender here
                    }
                }
                _ = interval.tick() => {
                    if let Some(client) = clients.next() {
                        if client.try_send((request.clone(), rsp_sender.clone())).is_err() {
                            wait_left -= 1;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        drop(rsp_sender);
        let end_err = if let Some(d) = self.each_timeout.checked_sub(self.retry_interval) {
            match tokio::time::timeout(d, rsp_receiver.recv()).await {
                Ok(Some(v)) => return v,
                Ok(None) => ResolvedRecord::failed(
                    domain,
                    self.negative_min_ttl,
                    ResolveDriverError::Internal("no response received".to_string()).into(),
                ),
                Err(_) => ResolvedRecord::failed(
                    domain,
                    self.negative_min_ttl,
                    ResolveDriverError::Timeout.into(),
                ),
            }
        } else {
            ResolvedRecord::failed(
                domain,
                self.negative_min_ttl,
                ResolveDriverError::Timeout.into(),
            )
        };
        last_err.unwrap_or(end_err)
    }
}
