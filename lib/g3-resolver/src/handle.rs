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

use std::future::{poll_fn, Future};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::sync::{mpsc, oneshot};

use super::{ArcResolvedRecord, ResolveLocalError, ResolvedRecordSource};
use crate::message::ResolveDriverRequest;

#[derive(Clone, Debug)]
pub struct ResolverHandle {
    req_sender: mpsc::UnboundedSender<ResolveDriverRequest>,
}

impl PartialEq for ResolverHandle {
    fn eq(&self, other: &Self) -> bool {
        self.req_sender.same_channel(&other.req_sender)
    }
}

impl ResolverHandle {
    pub(crate) fn new(req_sender: mpsc::UnboundedSender<ResolveDriverRequest>) -> Self {
        ResolverHandle { req_sender }
    }

    pub fn is_closed(&self) -> bool {
        self.req_sender.is_closed()
    }

    pub fn get_v4(&self, domain: String) -> Result<ResolveJob, ResolveLocalError> {
        let (sender, receiver) = oneshot::channel();
        let req = ResolveDriverRequest::GetV4(domain, sender);
        let sender = self.req_sender.clone();
        match sender.send(req) {
            Ok(_) => Ok(ResolveJob { receiver }),
            Err(_) => Err(ResolveLocalError::NoResolverRunning),
        }
    }

    pub fn get_v6(&self, domain: String) -> Result<ResolveJob, ResolveLocalError> {
        let (sender, receiver) = oneshot::channel();
        let req = ResolveDriverRequest::GetV6(domain, sender);
        let sender = self.req_sender.clone();
        match sender.send(req) {
            Ok(_) => Ok(ResolveJob { receiver }),
            Err(_) => Err(ResolveLocalError::NoResolverRunning),
        }
    }
}

pub struct ResolveJob {
    receiver: oneshot::Receiver<(ArcResolvedRecord, ResolvedRecordSource)>,
}
pub type ResolveJobRecvResult =
    Result<(ArcResolvedRecord, ResolvedRecordSource), ResolveLocalError>;

impl ResolveJob {
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<ResolveJobRecvResult> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(ret)) => Poll::Ready(Ok(ret)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(ResolveLocalError::NoResolverRunning)),
        }
    }

    pub async fn recv(&mut self) -> ResolveJobRecvResult {
        poll_fn(|cx| self.poll_recv(cx)).await
    }
}
