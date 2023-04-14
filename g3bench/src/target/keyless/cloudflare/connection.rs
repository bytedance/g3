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
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::task::{Context, Poll, Waker};

use anyhow::anyhow;
use concurrent_queue::{ConcurrentQueue, PushError};
use fxhash::FxBuildHasher;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use g3_io_ext::{LimitedReader, LimitedWriter};

pub(super) type BoxKeylessWriter = Box<dyn AsyncWrite + Send + Unpin>;
pub(super) type BoxKeylessReader = Box<dyn AsyncRead + Send + Unpin>;
pub(super) type BoxKeylessConnection = (BoxKeylessReader, BoxKeylessWriter);

pub(super) struct SavedKeylessConnection {
    pub(super) reader: LimitedReader<BoxKeylessReader>,
    pub(super) writer: LimitedWriter<BoxKeylessWriter>,
}

impl SavedKeylessConnection {
    pub(super) fn new(
        reader: LimitedReader<BoxKeylessReader>,
        writer: LimitedWriter<BoxKeylessWriter>,
    ) -> Self {
        SavedKeylessConnection { reader, writer }
    }
}

struct SharedState<T> {
    write_waker: RwLock<Option<Waker>>,
    req_queue: ConcurrentQueue<(T, Waker)>,
    rsp_table: Mutex<HashMap<u32, (T, Waker), FxBuildHasher>>,
}

impl<T> Default for SharedState<T> {
    fn default() -> Self {
        SharedState {
            write_waker: RwLock::new(None),
            req_queue: ConcurrentQueue::bounded(1024),
            rsp_table: Mutex::new(HashMap::with_hasher(FxBuildHasher::default())),
        }
    }
}

struct UnderlyingWriterState<T> {
    init: bool,
    shared: Arc<SharedState<T>>,
    current_buffer: Vec<u8>,
    current_offset: usize,
    current_request: Option<(T, Waker, u32)>,
    next_id: u32,
}

impl<T> UnderlyingWriterState<T>
where
    T: Unpin,
{
    fn poll_write<W>(
        &mut self,
        cx: &mut Context<'_>,
        mut writer: Pin<&mut W>,
    ) -> Poll<anyhow::Result<()>>
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
            if let Some((req, waker, id)) = self.current_request.take() {
                while self.current_offset < self.current_buffer.len() {
                    match writer
                        .as_mut()
                        .poll_write(cx, &self.current_buffer[self.current_offset..])
                    {
                        Poll::Ready(Ok(n)) => self.current_offset += n,
                        Poll::Ready(Err(e)) => {
                            self.shared.req_queue.close();
                            // TODO clean pending requests
                            return Poll::Ready(Err(anyhow!("connection error: {e:?}")));
                        }
                        Poll::Pending => {
                            self.current_request = Some((req, waker, id));
                            return Poll::Pending;
                        }
                    };
                }
                let mut rsp_table = self.shared.rsp_table.lock().unwrap();
                if let Some((req, waker)) = rsp_table.insert(id, (req, waker)) {
                    // TODO handle error
                }
            }

            let Ok((req, waker)) = self.shared.req_queue.pop() else {
                return Poll::Pending;
            };

            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            // TODO compose send buffer
            self.current_offset = 0;
            self.current_buffer.clear();
            self.current_request = Some((req, waker, id));
            // TODO fetch new request from queue
        }
    }
}

struct UnderlyingWriter<T, W> {
    writer: W,
    state: UnderlyingWriterState<T>,
}

impl<T, W> Future for UnderlyingWriter<T, W>
where
    W: AsyncWrite + Unpin,
    T: Unpin,
{
    type Output = anyhow::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.state.poll_write(cx, Pin::new(&mut me.writer))
    }
}

struct SendHandle<T> {
    shared: Arc<SharedState<T>>,
}

impl<T> SendHandle<T>
where
    T: Clone,
{
    fn send_request(&self, req: T) -> SendRequest<T> {
        SendRequest {
            shared: self.shared.clone(),
            request: Some(req),
        }
    }
}

#[derive(Clone)]
struct SendRequest<T>
where
    T: Clone,
{
    shared: Arc<SharedState<T>>,
    request: Option<T>,
}

impl<T> Future for SendRequest<T>
where
    T: Clone + Unpin,
{
    type Output = anyhow::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(req) = self.request.take() {
            let underlying_waker_guard = self.shared.write_waker.read().unwrap();
            if let Some(underlying_waker) = &*underlying_waker_guard {
                let rsp_waker = cx.waker().clone();
                match self.shared.req_queue.push((req, rsp_waker)) {
                    Ok(_) => {
                        underlying_waker.wake_by_ref();
                        Poll::Pending
                    }
                    Err(PushError::Closed(_)) => Poll::Ready(Err(anyhow!("connection closed"))),
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
            // TODO handle response
            todo!()
        }
    }
}

fn start_transfer<T>(connection: BoxKeylessConnection) -> SendHandle<T>
where
    T: Clone + Send + Unpin + 'static,
{
    let (r, w) = connection;
    let shared: Arc<SharedState<T>> = Arc::new(SharedState::default());

    let underlying_w = UnderlyingWriter {
        writer: w,
        state: UnderlyingWriterState {
            init: true,
            shared: Arc::clone(&shared),
            current_buffer: vec![],
            current_offset: 0,
            current_request: None,
            next_id: 0,
        },
    };
    tokio::spawn(underlying_w);

    SendHandle { shared }
}
