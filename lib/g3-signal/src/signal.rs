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

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::signal::unix::{signal, Signal, SignalKind};

pub enum SigResult {
    Continue,
    Break,
}

type SigAction = dyn Fn(u32) -> SigResult + Sync;

pub struct ActionSignal<'a> {
    signal: Signal,
    count: u32,
    action: &'a SigAction,
}

impl<'a> ActionSignal<'a> {
    pub fn new(signo: SignalKind, action: &'a SigAction) -> io::Result<Self> {
        Ok(ActionSignal {
            signal: signal(signo)?,
            count: 0,
            action,
        })
    }
}

impl<'a> Future for ActionSignal<'a> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match Pin::new(&mut self.signal).poll_recv(cx) {
                Poll::Ready(Some(_)) => {
                    self.count += 1;
                    match (self.action)(self.count) {
                        SigResult::Continue => continue,
                        SigResult::Break => return Poll::Ready(()),
                    }
                }
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
