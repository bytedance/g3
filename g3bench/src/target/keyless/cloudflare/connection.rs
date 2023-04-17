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

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::task::{Context, Poll, Waker};

use concurrent_queue::{ConcurrentQueue, PopError, PushError};
use fxhash::FxBuildHasher;
use tokio::io::{AsyncRead, AsyncWrite};

use super::{KeylessLocalError, KeylessRequest, KeylessResponse, KeylessResponseError};

struct ResponseValue {
    data: Option<KeylessResponse>,
    waker: Option<Waker>,
}

struct SharedState {
    write_waker: RwLock<Option<Waker>>,
    next_req_id: AtomicU32,
    req_queue: ConcurrentQueue<(KeylessRequest, Waker)>,
    rsp_table: Mutex<HashMap<u32, ResponseValue, FxBuildHasher>>,
    error: Mutex<Option<KeylessResponseError>>,
}

impl SharedState {
    fn next_req_id(&self) -> u32 {
        self.next_req_id.fetch_add(1, Ordering::Relaxed)
    }

    fn set_req_error(&self, e: io::Error) {
        let mut req_err_guard = self.error.lock().unwrap();
        *req_err_guard = Some(KeylessLocalError::WriteFailed(e).into());
    }

    fn set_rsp_error(&self, e: KeylessResponseError) {
        let mut rsp_err_guard = self.error.lock().unwrap();
        *rsp_err_guard = Some(e);
    }

    fn clean_pending_req(&self) {
        while let Ok((_r, waker)) = self.req_queue.pop() {
            waker.wake();
        }
        let mut rsp_table_guard = self.rsp_table.lock().unwrap();
        for (_, v) in rsp_table_guard.drain() {
            if let Some(waker) = v.waker {
                waker.wake();
            }
        }
    }
}

impl Default for SharedState {
    fn default() -> Self {
        SharedState {
            write_waker: RwLock::new(None),
            next_req_id: AtomicU32::new(0),
            req_queue: ConcurrentQueue::bounded(1024),
            rsp_table: Mutex::new(HashMap::with_hasher(FxBuildHasher::default())),
            error: Mutex::new(None),
        }
    }
}

struct UnderlyingWriterState {
    init: bool,
    shared: Arc<SharedState>,
    current_offset: usize,
    current_request: Option<(KeylessRequest, Waker)>,
}

impl UnderlyingWriterState {
    fn poll_write<W>(&mut self, cx: &mut Context<'_>, mut writer: Pin<&mut W>) -> Poll<()>
    where
        W: AsyncWrite + Unpin,
    {
        if self.init {
            // TODO use OnceLock
            let mut waker = self.shared.write_waker.write().unwrap();
            *waker = Some(cx.waker().clone());
            drop(waker);
            self.init = false;
        }

        loop {
            if let Some((req, waker)) = self.current_request.take() {
                let current_buffer = req.as_bytes();
                while self.current_offset < current_buffer.len() {
                    match writer
                        .as_mut()
                        .poll_write(cx, &current_buffer[self.current_offset..])
                    {
                        Poll::Ready(Ok(n)) => self.current_offset += n,
                        Poll::Ready(Err(e)) => {
                            self.shared.req_queue.close();
                            self.shared.clean_pending_req();
                            self.shared.set_req_error(e);
                            return Poll::Ready(());
                        }
                        Poll::Pending => {
                            self.current_request = Some((req, waker));
                            return Poll::Pending;
                        }
                    };
                }
                let mut rsp_table = self.shared.rsp_table.lock().unwrap();
                rsp_table.insert(
                    req.id(),
                    ResponseValue {
                        data: None,
                        waker: Some(waker),
                    },
                );
            }

            match self.shared.req_queue.pop() {
                Ok((req, waker)) => {
                    self.current_offset = 0;
                    self.current_request = Some((req, waker));
                }
                Err(PopError::Empty) => return Poll::Pending,
                Err(PopError::Closed) => return Poll::Ready(()),
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

#[derive(Clone)]
pub(crate) struct SendHandle {
    shared: Arc<SharedState>,
}

impl SendHandle {
    pub(crate) fn is_closed(&self) -> bool {
        self.shared.req_queue.is_closed()
    }

    pub(crate) fn send_request(&self, req: KeylessRequest) -> SendRequest {
        SendRequest {
            shared: self.shared.clone(),
            request: Some(req),
            rsp_id: 0,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SendRequest {
    shared: Arc<SharedState>,
    request: Option<KeylessRequest>,
    rsp_id: u32,
}

impl Future for SendRequest {
    type Output = Option<KeylessResponse>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(mut req) = self.request.take() {
            let underlying_waker_guard = self.shared.write_waker.read().unwrap();
            if let Some(underlying_waker) = &*underlying_waker_guard {
                let rsp_waker = cx.waker().clone();
                let id = self.shared.next_req_id();
                req.set_id(id);
                match self.shared.req_queue.push((req, rsp_waker)) {
                    Ok(_) => {
                        underlying_waker.wake_by_ref();
                        drop(underlying_waker_guard);
                        self.rsp_id = id;
                        Poll::Pending
                    }
                    Err(PushError::Closed(_)) => Poll::Ready(None),
                    Err(PushError::Full((req, waker))) => {
                        drop(underlying_waker_guard);
                        self.request = Some(req);
                        waker.wake();
                        Poll::Pending
                    }
                }
            } else {
                // wait underlying writer
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        } else {
            let mut rsp_table_guard = self.shared.rsp_table.lock().unwrap();
            let r = rsp_table_guard.remove(&self.rsp_id).and_then(|s| s.data);
            Poll::Ready(r)
        }
    }
}

pub(crate) fn start_transfer<R, W>(mut r: R, w: W) -> SendHandle
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let shared = Arc::new(SharedState::default());

    let underlying_w = UnderlyingWriter {
        writer: w,
        state: UnderlyingWriterState {
            init: true,
            shared: Arc::clone(&shared),
            current_offset: 0,
            current_request: None,
        },
    };
    tokio::spawn(underlying_w);
    let handle = SendHandle {
        shared: shared.clone(),
    };

    // TODO use a timer to clean timeout cache and keep hashtable small

    tokio::spawn(async move {
        let mut buf: Vec<u8> = Vec::with_capacity(512);
        loop {
            match KeylessResponse::read(&mut r, &mut buf).await {
                Ok(r) => {
                    let mut rsp_table_guard = shared.rsp_table.lock().unwrap();
                    let Some(entry) = rsp_table_guard.get_mut(&r.id()) else {
                        continue;
                    };
                    if let Some(waker) = entry.waker.take() {
                        entry.data = Some(r);
                        drop(rsp_table_guard);
                        waker.wake();
                    }
                }
                Err(e) => {
                    shared.req_queue.close();
                    shared.clean_pending_req();
                    shared.set_rsp_error(e);
                    break;
                }
            };
        }
    });

    handle
}
