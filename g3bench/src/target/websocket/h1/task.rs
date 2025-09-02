/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use bytes::Bytes;
use http::Method;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, BufReader};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use g3_http::client::HttpTransparentResponse;
use g3_io_ext::{LimitedBufReadExt, LimitedReader, LimitedWriteExt, LimitedWriter};
use g3_types::net::HttpUpgradeToken;

use super::{H1WebsocketArgs, WebsocketHistogramRecorder};
use crate::ProcArgs;
use crate::module::http::{HttpRuntimeStats, SavedHttpForwardConnection};
use crate::target::websocket::{ClientFrameBuilder, FrameType, ServerFrameHeader};
use crate::target::{BenchError, BenchTaskContext};

struct H1WebSocketTaskLoop {
    args: Arc<H1WebsocketArgs>,
    proc_args: Arc<ProcArgs>,
    reuse_conn_count: u64,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: WebsocketHistogramRecorder,

    request_buf: Vec<u8>,
    response_buf: Vec<u8>,
}

impl H1WebSocketTaskLoop {
    fn new(
        args: Arc<H1WebsocketArgs>,
        proc_args: Arc<ProcArgs>,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: WebsocketHistogramRecorder,
    ) -> Self {
        H1WebSocketTaskLoop {
            args,
            proc_args,
            reuse_conn_count: 0,
            runtime_stats,
            histogram_recorder,
            request_buf: Vec::new(),
            response_buf: Vec::new(),
        }
    }

    async fn upgrade<R, W>(&self, writer: &mut W, reader: &mut R) -> anyhow::Result<()>
    where
        R: AsyncBufRead + Send + Sync + Unpin,
        W: AsyncWrite + Send + Sync + Unpin,
    {
        let mut buf = Vec::with_capacity(512);
        let key = self
            .args
            .build_upgrade_request(&mut buf)
            .context("failed to build upgrade request")?;

        writer
            .write_all_flush(&buf)
            .await
            .map_err(|e| anyhow!("failed to write upgrade request: {e}"))?;

        let (rsp, _) = HttpTransparentResponse::parse(reader, &Method::GET, true, 1024).await?;
        if rsp.code != 101 {
            return Err(anyhow!(
                "upgrade failed, code: {}, reason: {}",
                rsp.code,
                rsp.reason
            ));
        }
        if !matches!(rsp.upgrade, Some(HttpUpgradeToken::Websocket)) {
            return Err(anyhow!(
                "no valid 'Upgrade' header found or 'Connection' contains no 'Upgrade'"
            ));
        }

        self.args
            .common
            .verify_upgrade_response_headers(key, rsp.end_to_end_headers.into())?;
        Ok(())
    }

    async fn new_connection(&mut self) -> anyhow::Result<SavedHttpForwardConnection> {
        self.runtime_stats.add_conn_attempt();
        let (r, w) = match tokio::time::timeout(
            self.args.common.connect_timeout,
            self.args.connect.new_http_connection(
                &self.args.common.target,
                &self.runtime_stats,
                &self.proc_args,
            ),
        )
        .await
        {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("timeout to get new connection")),
        };

        let r = LimitedReader::local_limited(
            r,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_south,
            self.runtime_stats.clone(),
        );
        let mut w = LimitedWriter::local_limited(
            w,
            self.proc_args.tcp_sock_speed_limit.shift_millis,
            self.proc_args.tcp_sock_speed_limit.max_north,
            self.runtime_stats.clone(),
        );

        let mut r = BufReader::new(r);
        tokio::time::timeout(
            self.args.common.upgrade_timeout,
            self.upgrade(&mut w, &mut r),
        )
        .await
        .map_err(|_| anyhow!("websocket upgrade timed out"))??;

        self.runtime_stats.add_conn_success();
        Ok(SavedHttpForwardConnection::new(r, w))
    }

    async fn read_frame_header<R>(&mut self, reader: &mut R) -> anyhow::Result<ServerFrameHeader>
    where
        R: AsyncBufRead + Unpin,
    {
        let buf = reader
            .fill_buf()
            .await
            .map_err(|e| anyhow!("failed to read frame from server: {e}"))?;

        let mut frame_header = if buf.len() < 2 {
            let mut buf = [0u8; 2];
            let nr = reader
                .read_exact(&mut buf)
                .await
                .map_err(|e| anyhow!("failed to read frame header: {e}"))?;
            if nr != buf.len() {
                return Err(anyhow!("not enough frame header data read"));
            }
            ServerFrameHeader::new(buf[0], buf[1]).context("invalid frame header received")?
        } else {
            let h =
                ServerFrameHeader::new(buf[0], buf[1]).context("invalid frame header received")?;
            reader.consume(2);
            h
        };

        if let Some(buf) = frame_header.payload_length_buf() {
            let nr = reader
                .read_exact(buf)
                .await
                .map_err(|e| anyhow!("failed to read payload length bytes: {e}"))?;
            if nr != buf.len() {
                return Err(anyhow!("not enough payload length bytes read"));
            }
            frame_header.parse_payload_length();
        }

        Ok(frame_header)
    }

    async fn recv_full_frame(
        &mut self,
        connection: &mut SavedHttpForwardConnection,
    ) -> anyhow::Result<FrameType> {
        self.response_buf.clear();
        let mut frame_type: Option<FrameType> = None;

        loop {
            let frame_header = self.read_frame_header(&mut connection.reader).await?;
            if frame_type.is_none() {
                if frame_header.frame_type() == FrameType::Continue {
                    return Err(anyhow!("the first frame type should not be Continue"));
                }
                frame_type = Some(frame_header.frame_type());
            } else if frame_header.frame_type() != FrameType::Continue {
                return Err(anyhow!(
                    "expected Continue frame type but we get {}",
                    frame_header.frame_type()
                ));
            }

            if frame_header.payload_length() > 0 {
                let Ok(to_read) = usize::try_from(frame_header.payload_length()) else {
                    return Err(anyhow!(
                        "too large frame payload length {}",
                        frame_header.payload_length()
                    ));
                };

                let nr = (&mut connection.reader)
                    .take(to_read as u64)
                    .read_to_end(&mut self.response_buf)
                    .await
                    .map_err(|e| anyhow!("failed to read payload: {e}"))?;
                if nr != to_read {
                    return Err(anyhow!(
                        "not enough payload data read: expected {to_read} but got {nr}"
                    ));
                }
            }

            if frame_header.is_last_frame() {
                break;
            }
        }

        frame_type.ok_or_else(|| anyhow!("no frame received"))
    }

    async fn run(
        mut self,
        mut req_receiver: mpsc::Receiver<(Bytes, oneshot::Sender<anyhow::Result<()>>)>,
    ) {
        loop {
            if req_receiver.is_closed() {
                break;
            }

            if self.reuse_conn_count > 0 {
                self.histogram_recorder
                    .record_conn_reuse_count(self.reuse_conn_count);
                self.reuse_conn_count = 0;
            }
            match self.new_connection().await {
                Ok(c) => {
                    if let Err(e) = self.run_with_connection(c, &mut req_receiver).await {
                        eprintln!("websocket connection task error: {e}");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("failed to create new websocket connection: {e}");
                    break;
                }
            }
        }

        if self.reuse_conn_count > 0 {
            self.histogram_recorder
                .record_conn_reuse_count(self.reuse_conn_count);
            self.reuse_conn_count = 0;
        }
    }

    async fn run_with_connection(
        &mut self,
        mut connection: SavedHttpForwardConnection,
        req_receiver: &mut mpsc::Receiver<(Bytes, oneshot::Sender<anyhow::Result<()>>)>,
    ) -> anyhow::Result<()> {
        let mut rsp_sender: Option<oneshot::Sender<anyhow::Result<()>>> = None;

        loop {
            tokio::select! {
                biased;

                r = req_receiver.recv() => {
                    let Some((data, sender)) = r else {
                        let builder = ClientFrameBuilder::new(FrameType::Close, self.args.common.max_frame_size);
                        self.request_buf.clear();
                        builder.build_frames(b"", &mut self.request_buf);
                        let _ = connection.writer.write_all_flush(&self.request_buf).await;
                        return Ok(());
                    };
                    connection.writer.write_all_flush(&data).await.map_err(|e| anyhow!("failed to write data: {e}"))?;
                    if let Some(sender) = rsp_sender.take() {
                        let _ = sender.send(Err(anyhow!("no response received")));
                    }
                    rsp_sender = Some(sender);
                }
                r = connection.reader.fill_wait_data() => {
                    let frame_type = match r {
                        Ok(true) => self.recv_full_frame(&mut connection).await?,
                        Ok(false) => {
                            return Err(anyhow!("connection closed without sending a Close frame"));
                        }
                        Err(e) => return Err(anyhow!("connection closed with error {e}")),
                    };

                    match frame_type {
                        FrameType::Continue => unreachable!(),
                        FrameType::Text => {}
                        FrameType::Binary => {}
                        FrameType::Close => {
                            let builder = ClientFrameBuilder::new(FrameType::Close, self.args.common.max_frame_size);
                            self.request_buf.clear();
                            builder.build_frames(&self.response_buf, &mut self.request_buf);
                            let _ = connection.writer.write_all_flush(&self.request_buf).await;
                            return Ok(());
                        }
                        FrameType::Ping => {
                            let builder = ClientFrameBuilder::new(FrameType::Pong, self.args.common.max_frame_size);
                            self.request_buf.clear();
                            builder.build_frames(&self.response_buf, &mut self.request_buf);
                            if let Err(e) = connection.writer.write_all_flush(&self.request_buf).await {
                                return Err(anyhow!("failed to write Pong frame: {e}"));
                            }
                            continue;
                        }
                        FrameType::Pong => {}
                    }

                    if let Some(sender) = rsp_sender.take() {
                        let r = self.args.common.verify_response_data(frame_type, &self.response_buf);
                        let _ = sender.send(r);
                    } else {
                        return Err(anyhow!("unexpected {frame_type} frame received"));
                    }
                }
            }
        }
    }
}

pub(super) struct H1WebsocketTaskContext {
    args: Arc<H1WebsocketArgs>,

    runtime_stats: Arc<HttpRuntimeStats>,
    histogram_recorder: WebsocketHistogramRecorder,

    request_buf: Vec<u8>,
    req_sender: mpsc::Sender<(Bytes, oneshot::Sender<anyhow::Result<()>>)>,
}

impl H1WebsocketTaskContext {
    pub(super) fn new(
        args: Arc<H1WebsocketArgs>,
        proc_args: Arc<ProcArgs>,
        runtime_stats: Arc<HttpRuntimeStats>,
        histogram_recorder: WebsocketHistogramRecorder,
    ) -> Self {
        let task_loop = H1WebSocketTaskLoop::new(
            args.clone(),
            proc_args,
            runtime_stats.clone(),
            histogram_recorder.clone(),
        );
        let (req_sender, req_receiver) = mpsc::channel(1);
        tokio::spawn(async move {
            task_loop.run(req_receiver).await;
        });

        H1WebsocketTaskContext {
            args,
            runtime_stats,
            histogram_recorder,
            request_buf: Vec::new(),
            req_sender,
        }
    }
}

impl BenchTaskContext for H1WebsocketTaskContext {
    fn mark_task_start(&self) {
        self.runtime_stats.add_task_total();
        self.runtime_stats.inc_task_alive();
    }

    fn mark_task_passed(&self) {
        self.runtime_stats.add_task_passed();
        self.runtime_stats.dec_task_alive();
    }

    fn mark_task_failed(&self) {
        self.runtime_stats.add_task_failed();
        self.runtime_stats.dec_task_alive();
    }

    async fn run(&mut self, _task_id: usize, time_started: Instant) -> Result<(), BenchError> {
        self.request_buf.clear();
        self.args.common.build_request_frames(&mut self.request_buf);

        let (rsp_sender, rsp_receiver) = oneshot::channel();
        self.req_sender
            .send((Bytes::copy_from_slice(&self.request_buf), rsp_sender))
            .await
            .map_err(|e| BenchError::Task(anyhow!("websocket task loop ended: {e}")))?;

        match tokio::time::timeout(self.args.common.timeout, rsp_receiver).await {
            Ok(Ok(Ok(_))) => {
                let total_time = time_started.elapsed();
                self.histogram_recorder.record_total_time(total_time);
                Ok(())
            }
            Ok(Ok(Err(e))) => Err(BenchError::Task(e)),
            Ok(Err(e)) => Err(BenchError::Task(anyhow!(
                "error when recv result from task loop: {e}"
            ))),
            Err(_) => Err(BenchError::Task(anyhow!(
                "timeout to recv result from task loop"
            ))),
        }
    }
}
