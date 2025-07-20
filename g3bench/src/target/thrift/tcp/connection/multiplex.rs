/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker, ready};
use std::time::Duration;

use atomic_waker::AtomicWaker;
use concurrent_queue::{ConcurrentQueue, PopError, PushError};
use rustc_hash::FxHashMap;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{Instant, Sleep};

use super::{
    ThriftTcpRequest, ThriftTcpResponse, ThriftTcpResponseError, ThriftTcpResponseLocalError,
};
use crate::target::thrift::tcp::ThriftTcpArgs;

struct ResponseValue {
    data: Option<ThriftTcpResponse>,
    waker: Option<Waker>,
    created: Instant,
    end: bool,
}

impl ResponseValue {
    fn new(waker: Waker) -> Self {
        ResponseValue {
            data: None,
            waker: Some(waker),
            created: Instant::now(),
            end: false,
        }
    }

    fn empty() -> Self {
        ResponseValue {
            data: None,
            waker: None,
            created: Instant::now(),
            end: true,
        }
    }
}

struct SharedState {
    args: Arc<ThriftTcpArgs>,
    write_waker: AtomicWaker,
    next_req_id: AtomicI32,
    req_queue: ConcurrentQueue<(ThriftTcpRequest, Waker)>,
    rsp_table: Mutex<FxHashMap<i32, ResponseValue>>,
    error: Mutex<Option<Arc<ThriftTcpResponseError>>>,
}

impl SharedState {
    fn new(args: Arc<ThriftTcpArgs>) -> Self {
        SharedState {
            args,
            write_waker: AtomicWaker::new(),
            next_req_id: AtomicI32::new(0),
            req_queue: ConcurrentQueue::bounded(1024),
            rsp_table: Mutex::new(FxHashMap::default()),
            error: Mutex::new(None),
        }
    }

    fn next_req_id(&self) -> i32 {
        self.next_req_id.fetch_add(1, Ordering::Relaxed)
    }

    fn set_local_error(&self, e: ThriftTcpResponseLocalError) {
        let mut req_err_guard = self.error.lock().unwrap();
        *req_err_guard = Some(Arc::new(ThriftTcpResponseError::Local(e)));
    }

    fn set_rsp_error(&self, e: ThriftTcpResponseError) {
        let mut rsp_err_guard = self.error.lock().unwrap();
        *rsp_err_guard = Some(Arc::new(e));
    }

    fn clean_pending_req(&self) {
        let mut rsp_table_guard = self.rsp_table.lock().unwrap();
        while let Ok((r, waker)) = self.req_queue.pop() {
            rsp_table_guard.insert(r.seq_id, ResponseValue::empty());
            waker.wake();
        }
        for v in (*rsp_table_guard).values_mut() {
            if let Some(waker) = v.waker.take() {
                waker.wake();
            }
            v.end = true;
        }
    }

    fn take_write_waker(&self) -> Option<Waker> {
        self.write_waker.take()
    }
}

struct UnderlyingWriterState {
    shared: Arc<SharedState>,
    current_offset: usize,
    current_request: Option<ThriftTcpRequest>,
    request_timeout: Duration,
    shutdown_wait: Option<Pin<Box<Sleep>>>,
}

impl UnderlyingWriterState {
    fn poll_write<W>(&mut self, cx: &mut Context<'_>, mut writer: Pin<&mut W>) -> Poll<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.shared.write_waker.register(cx.waker());

        let mut do_flush = false;
        loop {
            if let Some(req) = self.current_request.take() {
                let current_buffer = &req.buf;
                while self.current_offset < current_buffer.len() {
                    match writer
                        .as_mut()
                        .poll_write(cx, &current_buffer[self.current_offset..])
                    {
                        Poll::Ready(Ok(n)) => {
                            self.current_offset += n;
                            do_flush = true;
                        }
                        Poll::Ready(Err(e)) => {
                            self.shared.req_queue.close();
                            self.shared
                                .set_local_error(ThriftTcpResponseLocalError::WriteFailed(e));
                            self.shared.clean_pending_req();
                            let _ = writer.as_mut().poll_shutdown(cx);
                            return Poll::Ready(());
                        }
                        Poll::Pending => {
                            self.current_request = Some(req);
                            return Poll::Pending;
                        }
                    };
                }
            }

            match self.shared.req_queue.pop() {
                Ok((req, waker)) => {
                    let mut rsp_table_guard = self.shared.rsp_table.lock().unwrap();
                    rsp_table_guard.insert(req.seq_id, ResponseValue::new(waker));
                    self.current_offset = 0;
                    self.current_request = Some(req);
                }
                Err(PopError::Empty) => {
                    if do_flush {
                        if let Err(e) = ready!(writer.as_mut().poll_flush(cx)) {
                            self.shared.req_queue.close();
                            self.shared
                                .set_local_error(ThriftTcpResponseLocalError::WriteFailed(e));
                            self.shared.clean_pending_req();
                            let _ = writer.as_mut().poll_shutdown(cx);
                            return Poll::Ready(());
                        }
                    }
                    return Poll::Pending;
                }
                Err(PopError::Closed) => {
                    let _ = self.shared.take_write_waker(); // make sure no more wake by others
                    let mut sleep = self
                        .shutdown_wait
                        .take()
                        .unwrap_or_else(|| Box::pin(tokio::time::sleep(self.request_timeout)));
                    return match sleep.as_mut().poll(cx) {
                        Poll::Ready(_) => {
                            let _ = writer.as_mut().poll_shutdown(cx);
                            Poll::Ready(())
                        }
                        Poll::Pending => {
                            self.shutdown_wait = Some(sleep);
                            Poll::Pending
                        }
                    };
                }
            }
        }
    }
}

struct UnderlyingWriter<W> {
    writer: W,
    state: UnderlyingWriterState,
}

impl<W> Future for UnderlyingWriter<W>
where
    W: AsyncWrite + Unpin,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.state.poll_write(cx, Pin::new(&mut me.writer))
    }
}

pub(crate) struct SendRequest {
    shared: Arc<SharedState>,
    request_payload: Option<Arc<[u8]>>,
    rsp_id: i32,
}

impl Future for SendRequest {
    type Output = Result<ThriftTcpResponse, i32>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        use std::collections::hash_map::Entry;

        if let Some(req_payload) = self.request_payload.take() {
            let rsp_waker = cx.waker().clone();
            let id = self.shared.next_req_id();
            let req = match self.shared.args.build_tcp_request(id, &req_payload) {
                Ok(req) => req,
                Err(e) => {
                    self.shared
                        .set_local_error(ThriftTcpResponseLocalError::InvalidRequest(e));
                    return Poll::Ready(Err(id));
                }
            };
            match self.shared.req_queue.push((req, rsp_waker)) {
                Ok(_) => {
                    self.shared.write_waker.wake();
                    self.rsp_id = id;
                    Poll::Pending
                }
                Err(PushError::Closed(_)) => Poll::Ready(Err(self.rsp_id)),
                Err(PushError::Full((_req, waker))) => {
                    self.request_payload = Some(req_payload);
                    waker.wake();
                    Poll::Pending
                }
            }
        } else {
            let mut rsp_table_guard = self.shared.rsp_table.lock().unwrap();
            match rsp_table_guard.entry(self.rsp_id) {
                Entry::Occupied(v) => {
                    if v.get().end {
                        let v = v.remove();
                        Poll::Ready(v.data.ok_or(self.rsp_id))
                    } else {
                        Poll::Pending
                    }
                }
                Entry::Vacant(_) => Poll::Pending,
            }
        }
    }
}

pub(crate) struct MultiplexTransfer {
    shared: Arc<SharedState>,
    local_addr: SocketAddr,
}

impl Drop for MultiplexTransfer {
    fn drop(&mut self) {
        self.shared.req_queue.close();
        if let Some(waker) = self.shared.take_write_waker() {
            waker.wake(); // let the writer handle the quit
        }
    }
}

impl MultiplexTransfer {
    pub(crate) fn is_closed(&self) -> bool {
        self.shared.req_queue.is_closed()
    }

    #[inline]
    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub(crate) fn send_request(&self, req_payload: Arc<[u8]>) -> SendRequest {
        SendRequest {
            shared: self.shared.clone(),
            request_payload: Some(req_payload),
            rsp_id: 0,
        }
    }

    pub(crate) fn fetch_error(&self) -> Option<Arc<ThriftTcpResponseError>> {
        let guard = self.shared.error.lock().unwrap();
        guard.clone()
    }

    pub(crate) fn start<R, W>(
        args: Arc<ThriftTcpArgs>,
        mut r: R,
        w: W,
        local_addr: SocketAddr,
        request_timeout: Duration,
    ) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let shared = Arc::new(SharedState::new(args));
        let handle = MultiplexTransfer {
            shared: shared.clone(),
            local_addr,
        };

        let underlying_w = UnderlyingWriter {
            writer: w,
            state: UnderlyingWriterState {
                shared: Arc::clone(&shared),
                current_offset: 0,
                current_request: None,
                request_timeout,
                shutdown_wait: None,
            },
        };
        tokio::spawn(underlying_w);

        let clean_shared = shared.clone();
        tokio::spawn(async move {
            // use a timer to clean timeout cache and keep hashtable small
            let mut interval = tokio::time::interval(request_timeout);
            interval.tick().await;
            loop {
                interval.tick().await;

                let mut rsp_table_guard = clean_shared.rsp_table.lock().unwrap();
                rsp_table_guard.retain(|_, v| {
                    if v.created.elapsed() > request_timeout {
                        if let Some(waker) = v.waker.take() {
                            v.end = true;
                            waker.wake();
                        }
                        false
                    } else {
                        true
                    }
                });
            }
        });

        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::with_capacity(1024);
            loop {
                match shared.args.read_tcp_response(&mut r, &mut buf).await {
                    Ok(r) => {
                        let mut rsp_table_guard = shared.rsp_table.lock().unwrap();
                        let Some(entry) = rsp_table_guard.get_mut(&r.seq_id) else {
                            continue;
                        };
                        entry.end = true;
                        if let Some(waker) = entry.waker.take() {
                            entry.data = Some(r);
                            drop(rsp_table_guard);
                            waker.wake();
                        }
                    }
                    Err(e) => {
                        shared.req_queue.close();
                        shared.set_rsp_error(e);
                        shared.clean_pending_req();
                        if let Some(waker) = shared.take_write_waker() {
                            waker.wake(); // tell the writer to quit
                        }
                        break;
                    }
                };
            }
        });

        handle
    }
}
