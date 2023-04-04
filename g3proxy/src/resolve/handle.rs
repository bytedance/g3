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

use std::future::poll_fn;
use std::net::IpAddr;
use std::sync::Arc;
use std::task::{ready, Context, Poll};
use std::time::Duration;

use g3_resolver::{ResolveError, ResolvedRecordSource};
use g3_types::metrics::MetricsName;
use g3_types::resolve::{QueryStrategy, ResolveRedirectionValue, ResolveStrategy};

pub(crate) trait LoggedResolveJob {
    fn log_error(&self, _e: &ResolveError, _source: ResolvedRecordSource) {}
    fn poll_query(&mut self, cx: &mut Context<'_>) -> Poll<Result<Vec<IpAddr>, ResolveError>>;
}

pub(crate) type BoxLoggedResolveJob = Box<dyn LoggedResolveJob + Send + Sync>;

macro_rules! impl_logged_poll_query {
    () => {
        fn poll_query(&mut self, cx: &mut Context<'_>) -> Poll<Result<Vec<IpAddr>, ResolveError>> {
            match ready!(self.inner.poll_recv(cx)) {
                Ok((record, source)) => match &record.result {
                    Ok(addrs) => Poll::Ready(Ok(addrs.clone())),
                    Err(e) => {
                        self.log_error(e, source);
                        Poll::Ready(Err(e.clone()))
                    }
                },
                Err(e) => Poll::Ready(Err(e.into())),
            }
        }
    };
}

pub(crate) trait IntegratedResolverHandle {
    fn name(&self) -> &MetricsName;
    fn is_closed(&self) -> bool;
    fn query_v4(&self, domain: String) -> Result<BoxLoggedResolveJob, ResolveError>;
    fn query_v6(&self, domain: String) -> Result<BoxLoggedResolveJob, ResolveError>;

    fn clone_inner(&self) -> Option<g3_resolver::ResolverHandle>;
}

pub(crate) type ArcIntegratedResolverHandle = Arc<dyn IntegratedResolverHandle + Send + Sync>;

struct NeverResolveJob {}

impl LoggedResolveJob for NeverResolveJob {
    fn poll_query(&mut self, _cx: &mut Context<'_>) -> Poll<Result<Vec<IpAddr>, ResolveError>> {
        Poll::Pending
    }
}

pub(super) struct ErrorResolveJob {
    error: Option<ResolveError>,
}

impl ErrorResolveJob {
    pub(super) fn with_error(e: ResolveError) -> Self {
        ErrorResolveJob { error: Some(e) }
    }
}

impl LoggedResolveJob for ErrorResolveJob {
    fn poll_query(&mut self, _cx: &mut Context<'_>) -> Poll<Result<Vec<IpAddr>, ResolveError>> {
        if let Some(e) = self.error.take() {
            Poll::Ready(Err(e))
        } else {
            Poll::Ready(Ok(Vec::new()))
        }
    }
}

pub(crate) struct HappyEyeballsResolveJob {
    r1: Option<Vec<IpAddr>>,
    r2: Option<Vec<IpAddr>>,
    h1: BoxLoggedResolveJob,
    h2: BoxLoggedResolveJob,
    h1_done: bool,
    h2_done: bool,
    r2_block: bool,
    strategy: ResolveStrategy,
}

impl HappyEyeballsResolveJob {
    pub(crate) fn new_redirected(
        s: ResolveStrategy,
        h: &ArcIntegratedResolverHandle,
        v: ResolveRedirectionValue,
    ) -> Result<Self, ResolveError> {
        match v {
            ResolveRedirectionValue::Domain(d) => Self::new_dyn(s, h, &d),
            ResolveRedirectionValue::Ip((ip4, ip6)) => {
                let mut job = HappyEyeballsResolveJob {
                    r1: None,
                    r2: None,
                    h1: Box::new(NeverResolveJob {}),
                    h2: Box::new(NeverResolveJob {}),
                    h1_done: true,
                    h2_done: true,
                    r2_block: false,
                    strategy: s,
                };
                match s.query {
                    QueryStrategy::Ipv4Only => {
                        job.r1 = Some(ip4);
                        job.r2 = Some(Vec::new());
                    }
                    QueryStrategy::Ipv4First => {
                        if ip4.is_empty() {
                            job.r1 = Some(ip6);
                            job.r2 = Some(ip4);
                        } else {
                            job.r1 = Some(ip4);
                            job.r2 = Some(ip6);
                        }
                    }
                    QueryStrategy::Ipv6Only => {
                        job.r1 = Some(ip6);
                        job.r2 = Some(Vec::new());
                    }
                    QueryStrategy::Ipv6First => {
                        if ip6.is_empty() {
                            job.r1 = Some(ip4);
                            job.r2 = Some(ip6);
                        } else {
                            job.r1 = Some(ip6);
                            job.r2 = Some(ip4);
                        }
                    }
                }
                Ok(job)
            }
        }
    }

    pub(crate) fn new_dyn(
        s: ResolveStrategy,
        h: &ArcIntegratedResolverHandle,
        domain: &str,
    ) -> Result<Self, ResolveError> {
        if domain.is_empty() {
            return Err(ResolveError::EmptyDomain);
        }
        match s.query {
            QueryStrategy::Ipv4Only => {
                let h1 = h.query_v4(domain.to_string())?;
                let h2 = Box::new(NeverResolveJob {});
                Ok(HappyEyeballsResolveJob {
                    r1: None,
                    r2: None,
                    h1,
                    h2,
                    h1_done: false,
                    h2_done: true,
                    r2_block: false,
                    strategy: s,
                })
            }
            QueryStrategy::Ipv4First => {
                let h1 = h.query_v4(domain.to_string())?;
                let h2 = h.query_v6(domain.to_string())?;
                Ok(HappyEyeballsResolveJob {
                    r1: None,
                    r2: None,
                    h1,
                    h2,
                    h1_done: false,
                    h2_done: false,
                    r2_block: false,
                    strategy: s,
                })
            }
            QueryStrategy::Ipv6Only => {
                let h1 = h.query_v6(domain.to_string())?;
                let h2 = Box::new(NeverResolveJob {});
                Ok(HappyEyeballsResolveJob {
                    r1: None,
                    r2: None,
                    h1,
                    h2,
                    h1_done: false,
                    h2_done: true,
                    r2_block: false,
                    strategy: s,
                })
            }
            QueryStrategy::Ipv6First => {
                let h1 = h.query_v6(domain.to_string())?;
                let h2 = h.query_v4(domain.to_string())?;
                Ok(HappyEyeballsResolveJob {
                    r1: None,
                    r2: None,
                    h1,
                    h2,
                    h1_done: false,
                    h2_done: false,
                    r2_block: false,
                    strategy: s,
                })
            }
        }
    }

    async fn poll_h1_end(&mut self, max_count: usize) -> Result<Vec<IpAddr>, ResolveError> {
        match poll_fn(|cx| self.h1.poll_query(cx)).await {
            Ok(r1) => {
                self.h1_done = true;
                self.h1 = Box::new(NeverResolveJob {});
                Ok(self.strategy.pick_many(r1, max_count))
            }
            Err(e) => {
                self.h1_done = true;
                self.h1 = Box::new(NeverResolveJob {});
                Err(e)
            }
        }
    }

    async fn poll_h2_end(&mut self, max_count: usize) -> Result<Vec<IpAddr>, ResolveError> {
        match poll_fn(|cx| self.h2.poll_query(cx)).await {
            Ok(r2) => {
                self.h2_done = true;
                self.h2 = Box::new(NeverResolveJob {});
                Ok(self.strategy.pick_many(r2, max_count))
            }
            Err(e) => {
                self.h2_done = true;
                self.h2 = Box::new(NeverResolveJob {});
                Err(e)
            }
        }
    }

    pub(crate) async fn get_r1_or_first(
        &mut self,
        resolution_delay: Duration,
        max_count: usize,
    ) -> Result<Vec<IpAddr>, ResolveError> {
        if let Some(r1) = self.r1.take() {
            assert!(self.h1_done);
            // h1 should be a never job
            return Ok(self.strategy.pick_many(r1, max_count));
        }

        if self.h2_done {
            return self.poll_h1_end(max_count).await;
        }

        tokio::select! {
            biased;

            r = poll_fn(|cx| self.h1.poll_query(cx)) => {
                match r {
                    Ok(r1) => {
                        self.h1_done = true;
                        self.h1 = Box::new(NeverResolveJob {});
                        Ok(self.strategy.pick_many(r1, max_count))
                    }
                    Err(e) => {
                        self.h1 = Box::new(ErrorResolveJob::with_error(e));
                        self.poll_h2_end(max_count).await
                    }
                }
            }
            r = poll_fn(|cx| self.h2.poll_query(cx)) => {
                match r {
                    Ok(r2) => {
                        self.h2_done = true;
                        self.h2 = Box::new(NeverResolveJob {});

                        if r2.is_empty() {
                            self.r2 = Some(r2);
                            self.poll_h1_end(max_count).await
                        } else {
                            match tokio::time::timeout(resolution_delay, poll_fn(|cx| self.h1.poll_query(cx)))
                                .await
                            {
                                Ok(Ok(r1)) => {
                                    self.r2 = Some(r2);
                                    self.h1_done = true;
                                    self.h1 = Box::new(NeverResolveJob {});
                                    Ok(self.strategy.pick_many(r1, max_count))
                                }
                                Ok(Err(e)) => {
                                    self.h1 = Box::new(ErrorResolveJob::with_error(e));
                                    Ok(self.strategy.pick_many(r2, max_count))
                                }
                                Err(_) => Ok(self.strategy.pick_many(r2, max_count)),
                            }
                        }
                    }
                    Err(e) => {
                        self.h2 = Box::new(ErrorResolveJob::with_error(e));
                        self.poll_h1_end(max_count).await
                    }
                }
            }
        }
    }

    pub(crate) async fn get_r2_or_never(
        &mut self,
        max_count: usize,
    ) -> Result<Vec<IpAddr>, ResolveError> {
        if self.r2_block {
            // make sure call get_r2_or_never again will block
            return poll_fn(|cx| NeverResolveJob {}.poll_query(cx)).await;
        }

        if let Some(r2) = self.r2.take() {
            self.r2_block = true;
            return Ok(self.strategy.pick_many(r2, max_count));
        }

        // there must be at most 1 query at r2 stage
        let r = if !self.h2_done {
            poll_fn(|cx| self.h2.poll_query(cx))
                .await
                .map(|r2| self.strategy.pick_many(r2, max_count))
        } else if !self.h1_done {
            poll_fn(|cx| self.h1.poll_query(cx))
                .await
                .map(|r1| self.strategy.pick_many(r1, max_count))
        } else {
            // if all done, return empty record to make caller know it
            Ok(Vec::new())
        };
        self.r2_block = true;
        r
    }
}

enum ArriveFirstResolveJobInner {
    OnlyOne(BoxLoggedResolveJob),
    First(BoxLoggedResolveJob, BoxLoggedResolveJob),
}

pub(crate) struct ArriveFirstResolveJob {
    strategy: ResolveStrategy,
    inner: Option<ArriveFirstResolveJobInner>,
}

impl ArriveFirstResolveJob {
    pub(crate) fn new(
        handle: &ArcIntegratedResolverHandle,
        strategy: ResolveStrategy,
        domain: &str,
    ) -> Result<Self, ResolveError> {
        if domain.is_empty() {
            return Err(ResolveError::EmptyDomain);
        }
        let inner = match strategy.query {
            QueryStrategy::Ipv4Only => {
                ArriveFirstResolveJobInner::OnlyOne(handle.query_v4(domain.to_string())?)
            }
            QueryStrategy::Ipv6Only => {
                ArriveFirstResolveJobInner::OnlyOne(handle.query_v6(domain.to_string())?)
            }
            QueryStrategy::Ipv4First => ArriveFirstResolveJobInner::First(
                handle.query_v4(domain.to_string())?,
                handle.query_v6(domain.to_string())?,
            ),
            QueryStrategy::Ipv6First => ArriveFirstResolveJobInner::First(
                handle.query_v6(domain.to_string())?,
                handle.query_v4(domain.to_string())?,
            ),
        };
        Ok(ArriveFirstResolveJob {
            strategy,
            inner: Some(inner),
        })
    }

    pub(crate) fn poll_all_addrs(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Vec<IpAddr>, ResolveError>> {
        match self.inner.take() {
            Some(inner) => match inner {
                ArriveFirstResolveJobInner::OnlyOne(mut job) => match job.poll_query(cx) {
                    Poll::Pending => {
                        self.inner = Some(ArriveFirstResolveJobInner::OnlyOne(job));
                        Poll::Pending
                    }
                    Poll::Ready(t) => Poll::Ready(t),
                },
                ArriveFirstResolveJobInner::First(mut job1, mut job2) => {
                    match job1.poll_query(cx) {
                        Poll::Ready(Ok(t)) => {
                            if t.is_empty() {
                                self.inner = Some(ArriveFirstResolveJobInner::OnlyOne(job2));
                                self.poll_all_addrs(cx)
                            } else {
                                Poll::Ready(Ok(t))
                            }
                        }
                        Poll::Ready(Err(_)) => {
                            self.inner = Some(ArriveFirstResolveJobInner::OnlyOne(job2));
                            self.poll_all_addrs(cx)
                        }
                        Poll::Pending => match job2.poll_query(cx) {
                            Poll::Ready(Ok(t)) => {
                                if t.is_empty() {
                                    self.inner = Some(ArriveFirstResolveJobInner::OnlyOne(job1));
                                    Poll::Pending
                                } else {
                                    Poll::Ready(Ok(t))
                                }
                            }
                            Poll::Ready(Err(_)) => {
                                self.inner = Some(ArriveFirstResolveJobInner::OnlyOne(job1));
                                Poll::Pending
                            }
                            Poll::Pending => {
                                self.inner = Some(ArriveFirstResolveJobInner::First(job1, job2));
                                Poll::Pending
                            }
                        },
                    }
                }
            },
            None => Poll::Ready(Ok(vec![])),
        }
    }

    pub(crate) fn poll_best_addr(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<IpAddr, ResolveError>> {
        let ips = ready!(self.poll_all_addrs(cx))?;
        let ip = self.strategy.pick_best(ips).ok_or_else(|| {
            ResolveError::UnexpectedError(
                "resolver job return ok but with no ip can be selected".to_string(),
            )
        })?;
        Poll::Ready(Ok(ip))
    }
}
