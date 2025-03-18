/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_daemon::server::ServerQuitPolicy;
use g3_io_ext::{IdleInterval, LimitedCopy, LimitedCopyConfig, LimitedCopyError, OptionalInterval};

use crate::serve::{ServerTaskError, ServerTaskResult};

pub(crate) trait StreamTransitTask {
    fn copy_config(&self) -> LimitedCopyConfig;
    fn idle_check_interval(&self) -> IdleInterval;
    fn max_idle_count(&self) -> usize;
    fn log_client_shutdown(&self);
    fn log_upstream_shutdown(&self);
    fn log_periodic(&self);
    fn log_flush_interval(&self) -> Option<Duration>;
    fn quit_policy(&self) -> &ServerQuitPolicy;

    async fn transit_transparent<CR, CW, UR, UW>(
        &self,
        mut clt_r: CR,
        mut clt_w: CW,
        mut ups_r: UR,
        mut ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let copy_config = self.copy_config();
        let clt_to_ups = LimitedCopy::new(&mut clt_r, &mut ups_w, &copy_config);
        let ups_to_clt = LimitedCopy::new(&mut ups_r, &mut clt_w, &copy_config);

        self.transit_transparent2(clt_to_ups, ups_to_clt).await
    }

    async fn transit_transparent2<CR, CW, UR, UW>(
        &self,
        mut clt_to_ups: LimitedCopy<'_, CR, UW>,
        mut ups_to_clt: LimitedCopy<'_, UR, CW>,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut idle_interval = self.idle_check_interval();
        let mut log_interval = self
            .log_flush_interval()
            .map(|log_interval| {
                let interval =
                    tokio::time::interval_at(Instant::now() + log_interval, log_interval);
                OptionalInterval::with(interval)
            })
            .unwrap_or_default();
        let mut idle_count = 0;
        let max_idle_count = self.max_idle_count();
        loop {
            tokio::select! {
                r = &mut clt_to_ups => {
                    return match r {
                        Ok(_) => {
                            let _ = clt_to_ups.writer().shutdown().await;
                            self.log_client_shutdown();
                            self.transit_south(ups_to_clt, log_interval, idle_interval, idle_count, max_idle_count).await
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            let _ = ups_to_clt.write_flush().await;
                            Err(ServerTaskError::UpstreamWriteFailed(e))
                        }
                    };
                }
                r = &mut ups_to_clt => {
                    return match r {
                        Ok(_) => {
                            let _ = ups_to_clt.writer().shutdown().await;
                            self.log_upstream_shutdown();
                            self.transit_north(clt_to_ups, log_interval, idle_interval, idle_count, max_idle_count).await
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            let _ = clt_to_ups.write_flush().await;
                            Err(ServerTaskError::ClientTcpWriteFailed(e))
                        }
                    };
                }
                _ = log_interval.tick() => {
                    self.log_periodic();
                }
                n = idle_interval.tick() => {
                    if clt_to_ups.is_idle() && ups_to_clt.is_idle() {
                        idle_count += n;

                        if idle_count >= max_idle_count {
                            return Err(ServerTaskError::Idle(idle_interval.period(), idle_count));
                        }
                    } else {
                        idle_count = 0;

                        clt_to_ups.reset_active();
                        ups_to_clt.reset_active();
                    }

                    if self.quit_policy().force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }

    async fn transit_north<CR, UW>(
        &self,
        mut clt_to_ups: LimitedCopy<'_, CR, UW>,
        mut log_interval: OptionalInterval,
        mut idle_interval: IdleInterval,
        mut idle_count: usize,
        max_idle_count: usize,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        loop {
            tokio::select! {
                r = &mut clt_to_ups => {
                    return match r {
                        Ok(_) => {
                            let _ = clt_to_ups.writer().shutdown().await;
                            Ok(())
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::UpstreamWriteFailed(e)),
                    };
                }
                _ = log_interval.tick() => {
                    self.log_periodic();
                }
                n = idle_interval.tick() => {
                    if clt_to_ups.is_idle() {
                        idle_count += n;

                        if idle_count >= max_idle_count {
                            return Err(ServerTaskError::Idle(idle_interval.period(), idle_count));
                        }
                    } else {
                        idle_count = 0;

                        clt_to_ups.reset_active();
                    }

                    if self.quit_policy().force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }

    async fn transit_south<CW, UR>(
        &self,
        mut ups_to_clt: LimitedCopy<'_, UR, CW>,
        mut log_interval: OptionalInterval,
        mut idle_interval: IdleInterval,
        mut idle_count: usize,
        max_idle_count: usize,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        loop {
            tokio::select! {
                r = &mut ups_to_clt => {
                    return match r {
                        Ok(_) => {
                            let _ = ups_to_clt.writer().shutdown().await;
                            Ok(())
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = log_interval.tick() => {
                    self.log_periodic();
                }
                n = idle_interval.tick() => {
                    if ups_to_clt.is_idle() {
                        idle_count += n;

                        if idle_count >= max_idle_count {
                            return Err(ServerTaskError::Idle(idle_interval.period(), idle_count));
                        }
                    } else {
                        idle_count = 0;

                        ups_to_clt.reset_active();
                    }

                    if self.quit_policy().force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }
}
