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

use super::{ThriftTcpResponse, ThriftTcpResponseError};
use crate::target::thrift::tcp::ThriftTcpArgs;
use crate::target::thrift::tcp::header::HeaderBufOffsets;

struct QueuedRequest {
    seq_id: i32,
    payload: Arc<[u8]>,
    rsp_waker: Waker,
}

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
    req_queue: ConcurrentQueue<QueuedRequest>,
    rsp_table: Mutex<FxHashMap<i32, ResponseValue>>,
    error: Mutex<Option<Arc<ThriftTcpResponseError>>>,
}

impl SharedState {
    fn new(args: Arc<ThriftTcpArgs>) -> Self {
        SharedState {
            args,
            write_waker: AtomicWaker::new(),
            req_queue: ConcurrentQueue::bounded(1024),
            rsp_table: Mutex::new(FxHashMap::default()),
            error: Mutex::new(None),
        }
    }

    fn set_local_error(&self, e: ThriftTcpResponseError) {
        let mut req_err_guard = self.error.lock().unwrap();
        *req_err_guard = Some(Arc::new(e));
    }

    fn set_rsp_error(&self, e: ThriftTcpResponseError) {
        let mut rsp_err_guard = self.error.lock().unwrap();
        *rsp_err_guard = Some(Arc::new(e));
    }

    fn clean_pending_req(&self) {
        let mut rsp_table_guard = self.rsp_table.lock().unwrap();
        while let Ok(r) = self.req_queue.pop() {
            rsp_table_guard.insert(r.seq_id, ResponseValue::empty());
            r.rsp_waker.wake();
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

    send_buf: Vec<u8>,
    send_buf_write_offset: usize,
    send_header_size: usize,
    send_header_buf_offsets: Option<HeaderBufOffsets>,

    request_timeout: Duration,
    shutdown_wait: Option<Pin<Box<Sleep>>>,
}

impl UnderlyingWriterState {
    fn new(shared: Arc<SharedState>, request_timeout: Duration) -> anyhow::Result<Self> {
        let mut send_buf = Vec::with_capacity(1024);
        let mut send_header_buf_offsets = None;
        if let Some(header_builder) = &shared.args.header_builder {
            let offsets = header_builder.build(
                shared.args.global.request_builder.protocol(),
                0,
                &mut send_buf,
            )?;
            send_header_buf_offsets = Some(offsets);
        }
        let send_header_size = send_buf.len();
        let send_buf_write_offset = send_buf.len();

        Ok(UnderlyingWriterState {
            shared,
            send_buf,
            send_buf_write_offset,
            send_header_size,
            send_header_buf_offsets,
            request_timeout,
            shutdown_wait: None,
        })
    }

    fn build_new_request(&mut self, seq_id: i32, payload: &[u8]) -> anyhow::Result<()> {
        if let Some(offsets) = &self.send_header_buf_offsets {
            offsets.update_seq_id(&mut self.send_buf, seq_id)?;

            self.send_buf.resize(self.send_header_size, 0);
            self.shared.args.global.request_builder.build_call(
                seq_id,
                self.shared.args.framed,
                payload,
                &mut self.send_buf,
            )?;

            offsets.update_length(&mut self.send_buf)?;
        } else {
            self.send_buf.clear();
            self.shared.args.global.request_builder.build_call(
                seq_id,
                self.shared.args.framed,
                payload,
                &mut self.send_buf,
            )?;
        }

        self.send_buf_write_offset = 0;
        Ok(())
    }

    fn poll_write<W>(&mut self, cx: &mut Context<'_>, mut writer: Pin<&mut W>) -> Poll<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.shared.write_waker.register(cx.waker());

        let mut do_flush = false;
        loop {
            while self.send_buf_write_offset < self.send_buf.len() {
                match writer
                    .as_mut()
                    .poll_write(cx, &self.send_buf[self.send_buf_write_offset..])
                {
                    Poll::Ready(Ok(n)) => {
                        self.send_buf_write_offset += n;
                        do_flush = true;
                    }
                    Poll::Ready(Err(e)) => {
                        self.shared.req_queue.close();
                        self.shared
                            .set_local_error(ThriftTcpResponseError::WriteFailed(e));
                        self.shared.clean_pending_req();
                        let _ = writer.as_mut().poll_shutdown(cx);
                        return Poll::Ready(());
                    }
                    Poll::Pending => return Poll::Pending,
                };
            }

            match self.shared.req_queue.pop() {
                Ok(req) => {
                    if let Err(e) = self.build_new_request(req.seq_id, &req.payload) {
                        self.shared.req_queue.close();
                        self.shared
                            .set_local_error(ThriftTcpResponseError::InvalidRequest(e));
                        self.shared.clean_pending_req();
                        let _ = writer.as_mut().poll_shutdown(cx);
                        return Poll::Ready(());
                    }

                    let mut rsp_table_guard = self.shared.rsp_table.lock().unwrap();
                    rsp_table_guard.insert(req.seq_id, ResponseValue::new(req.rsp_waker));
                }
                Err(PopError::Empty) => {
                    if do_flush {
                        if let Err(e) = ready!(writer.as_mut().poll_flush(cx)) {
                            self.shared.req_queue.close();
                            self.shared
                                .set_local_error(ThriftTcpResponseError::WriteFailed(e));
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
            let req = QueuedRequest {
                seq_id: self.rsp_id,
                payload: req_payload,
                rsp_waker: cx.waker().clone(),
            };
            match self.shared.req_queue.push(req) {
                Ok(_) => {
                    self.shared.write_waker.wake();
                    Poll::Pending
                }
                Err(PushError::Closed(_)) => Poll::Ready(Err(self.rsp_id)),
                Err(PushError::Full(req)) => {
                    self.request_payload = Some(req.payload);
                    req.rsp_waker.wake();
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
    next_req_id: AtomicI32,
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

    fn next_req_id(&self) -> i32 {
        let id = self.next_req_id.fetch_add(1, Ordering::Relaxed);
        if id == 0 { self.next_req_id() } else { id }
    }

    pub(crate) fn send_request(&self, req_payload: Arc<[u8]>) -> SendRequest {
        SendRequest {
            shared: self.shared.clone(),
            request_payload: Some(req_payload),
            rsp_id: self.next_req_id(),
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
    ) -> anyhow::Result<Self>
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let shared = Arc::new(SharedState::new(args));
        let handle = MultiplexTransfer {
            shared: shared.clone(),
            next_req_id: AtomicI32::new(0),
            local_addr,
        };

        let state = UnderlyingWriterState::new(shared.clone(), request_timeout)?;
        tokio::spawn(UnderlyingWriter { writer: w, state });

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

        Ok(handle)
    }
}
