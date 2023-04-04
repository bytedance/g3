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
use std::io;
use std::net::{self, SocketAddr};
use std::task::{Context, Poll};

use futures_util::FutureExt;
use tokio::net::{TcpListener, TcpStream};

pub struct LimitedTcpListener {
    inner: TcpListener,
    offline: bool,
    accept_again: bool,
}

impl LimitedTcpListener {
    pub fn new(listener: TcpListener) -> Self {
        LimitedTcpListener {
            inner: listener,
            offline: false,
            accept_again: false,
        }
    }

    pub fn from_std(listener: net::TcpListener) -> io::Result<Self> {
        Ok(LimitedTcpListener::new(TcpListener::from_std(listener)?))
    }

    pub fn set_offline(&mut self) -> bool {
        // TODO do something to stop the listen queue after kernel support it
        self.offline = true;
        self.accept_again
    }

    pub async fn accept(&mut self) -> io::Result<Option<(TcpStream, SocketAddr, SocketAddr)>> {
        if let Some((stream, peer_addr)) = poll_fn(|cx| self.poll_accept(cx)).await? {
            let local_addr = stream.local_addr()?;
            Ok(Some((stream, peer_addr, local_addr)))
        } else {
            Ok(None)
        }
    }

    fn poll_accept(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Option<(TcpStream, SocketAddr)>>> {
        match self.inner.poll_accept(cx)? {
            Poll::Ready((stream, addr)) => {
                self.accept_again = true;
                Poll::Ready(Ok(Some((stream, addr))))
            }
            Poll::Pending => {
                self.accept_again = false;
                if self.offline {
                    Poll::Ready(Ok(None))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    pub async fn accept_current_available<E, F>(
        &mut self,
        r: io::Result<Option<(TcpStream, SocketAddr, SocketAddr)>>,
        accept: &F,
    ) -> Result<(), E>
    where
        F: Fn(io::Result<Option<(TcpStream, SocketAddr, SocketAddr)>>) -> Result<(), E>,
    {
        accept(r)?;
        for _ in 1..100 {
            let Some(r) = self.accept().now_or_never() else {
                return Ok(());
            };
            accept(r)?;
        }
        Ok(())
    }
}
