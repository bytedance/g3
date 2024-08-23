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

use anyhow::anyhow;
use quinn::{RecvStream, SendStream};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time::Instant;

use g3_io_ext::{LimitedCopy, LimitedCopyError};

use super::StreamDetourContext;
use crate::config::server::ServerConfig;
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) struct StreamDetourStream {
    pub(super) north_send: SendStream,
    pub(super) north_recv: RecvStream,
    pub(super) south_send: SendStream,
    pub(super) south_recv: RecvStream,
    pub(super) force_quit_sender: mpsc::Sender<()>,
}

impl<'a, SC> StreamDetourContext<'a, SC>
where
    SC: ServerConfig,
{
    pub(super) async fn relay<CR, CW, UR, UW>(
        self,
        mut clt_r: CR,
        mut clt_w: CW,
        mut ups_r: UR,
        mut ups_w: UW,
        d_stream: StreamDetourStream,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let StreamDetourStream {
            mut north_send,
            mut north_recv,
            mut south_send,
            mut south_recv,
            force_quit_sender,
        } = d_stream;

        let copy_config = self.server_config.limited_copy_config();

        let mut clt_to_d = LimitedCopy::new(&mut clt_r, &mut north_send, &copy_config);
        let mut d_to_ups = LimitedCopy::new(&mut north_recv, &mut ups_w, &copy_config);
        let mut ups_to_d = LimitedCopy::new(&mut ups_r, &mut south_send, &copy_config);
        let mut d_to_clt = LimitedCopy::new(&mut south_recv, &mut clt_w, &copy_config);

        let idle_duration = self.server_config.task_idle_check_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                r = &mut clt_to_d => {
                    return match r {
                        Ok(_) => {
                            self.relay_after_client_closed(north_send, south_send, d_to_ups).await;
                            Err(ServerTaskError::ClosedByClient)
                        },
                        Err(LimitedCopyError::ReadFailed(e)) => {
                            self.relay_after_client_closed(north_send, south_send, d_to_ups).await;
                            Err(ServerTaskError::ClientTcpReadFailed(e))
                        },
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            self.relay_after_detour_failed(south_send, d_to_ups, d_to_clt).await;
                            Err(
                                ServerTaskError::InternalAdapterError(
                                    anyhow!("write client data to detour service failed: {e}")
                                )
                            )
                        },
                    };
                }
                r = &mut d_to_ups => {
                    return match r {
                        Ok(_) => {
                            self.relay_after_detour_failed(south_send, d_to_ups, d_to_clt).await;
                            Err(
                                ServerTaskError::InternalAdapterError(
                                    anyhow!("client connection closed by detour service")
                                )
                            )
                        },
                        Err(LimitedCopyError::ReadFailed(e)) => {
                            self.relay_after_detour_failed(south_send, d_to_ups, d_to_clt).await;
                            Err(
                                ServerTaskError::InternalAdapterError(
                                    anyhow!("read client data from detour service failed: {e}"),
                                )
                            )
                        },
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            self.relay_after_remote_closed(north_send, south_send, d_to_clt).await;
                            Err(ServerTaskError::UpstreamWriteFailed(e))
                        },
                    };
                }
                r = &mut ups_to_d => {
                    return match r {
                        Ok(_) => {
                            self.relay_after_remote_closed(north_send, south_send, d_to_clt).await;
                            Err(ServerTaskError::ClosedByUpstream)
                        },
                        Err(LimitedCopyError::ReadFailed(e)) => {
                            self.relay_after_remote_closed(north_send, south_send, d_to_clt).await;
                            Err(ServerTaskError::UpstreamReadFailed(e))
                        },
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            self.relay_after_detour_failed(north_send, d_to_ups, d_to_clt).await;
                            Err(
                                ServerTaskError::InternalAdapterError(
                                    anyhow!("write remote data to detour service failed: {e}"),
                                )
                            )
                        },
                    };
                }
                r = &mut d_to_clt => {
                    return match r {
                        Ok(_) => {
                            self.relay_after_detour_failed(north_send, d_to_ups, d_to_clt).await;
                            Err(
                                ServerTaskError::InternalAdapterError(
                                    anyhow!("remote connection closed by detour service")
                                )
                            )
                        },
                        Err(LimitedCopyError::ReadFailed(e)) => {
                            self.relay_after_detour_failed(north_send, d_to_ups, d_to_clt).await;
                            Err(
                                ServerTaskError::InternalAdapterError(
                                    anyhow!("read remote data from detour service failed: {e}"),
                                )
                            )
                        },
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            self.relay_after_client_closed(north_send, south_send, d_to_ups).await;
                            Err(ServerTaskError::ClientTcpWriteFailed(e))
                        },
                    };
                }
                _ = idle_interval.tick() => {
                    if clt_to_d.is_idle() && d_to_clt.is_idle() && ups_to_d.is_idle() && d_to_ups.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user) = self.user {
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.server_config.task_max_idle_count()
                        };

                        if quit {
                            return Err(ServerTaskError::Idle(idle_duration, idle_count));
                        }
                    } else {
                        idle_count = 0;

                        clt_to_d.reset_active();
                        d_to_ups.reset_active();
                        ups_to_d.reset_active();
                        d_to_clt.reset_active();
                    }

                    if let Some(user) = self.user {
                        if user.is_blocked() {
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.server_quit_policy.force_quit() {
                        let _ = force_quit_sender.try_send(());
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            };
        }
    }

    async fn relay_after_client_closed<UW>(
        self,
        mut north_send: SendStream,
        mut south_send: SendStream,
        mut d_to_ups: LimitedCopy<'_, RecvStream, UW>,
    ) where
        UW: AsyncWrite + Unpin,
    {
        let _ = south_send.finish();
        let shutdown = match north_send.finish() {
            Ok(_) => (&mut d_to_ups).await.is_ok(),
            Err(_) => d_to_ups.write_flush().await.is_ok(),
        };
        if shutdown {
            let _ = d_to_ups.writer().shutdown().await;
        }
    }

    async fn relay_after_remote_closed<CW>(
        self,
        mut north_send: SendStream,
        mut south_send: SendStream,
        mut d_to_clt: LimitedCopy<'_, RecvStream, CW>,
    ) where
        CW: AsyncWrite + Unpin,
    {
        let _ = north_send.finish();
        let shutdown = match south_send.finish() {
            Ok(_) => (&mut d_to_clt).await.is_ok(),
            Err(_) => d_to_clt.write_flush().await.is_ok(),
        };
        if shutdown {
            let _ = d_to_clt.writer().shutdown().await;
        }
    }

    async fn relay_after_detour_failed<CW, UW>(
        self,
        mut left_sender: SendStream,
        mut d_to_ups: LimitedCopy<'_, RecvStream, UW>,
        mut d_to_clt: LimitedCopy<'_, RecvStream, CW>,
    ) where
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let _ = left_sender.finish();
        if d_to_ups.write_flush().await.is_ok() {
            let _ = d_to_ups.writer().shutdown().await;
        }
        if d_to_clt.write_flush().await.is_ok() {
            let _ = d_to_clt.writer().shutdown().await;
        }
    }
}
