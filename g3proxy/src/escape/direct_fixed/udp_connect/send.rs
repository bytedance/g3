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

use std::task::{ready, Context, Poll};

use g3_io_ext::{AsyncUdpSend, UdpCopyRemoteError, UdpCopyRemoteSend};

pub(super) struct DirectUdpConnectRemoteSend<T> {
    inner: T,
}

impl<T> DirectUdpConnectRemoteSend<T>
where
    T: AsyncUdpSend,
{
    pub(super) fn new(send: T) -> Self {
        DirectUdpConnectRemoteSend { inner: send }
    }
}

impl<T> UdpCopyRemoteSend for DirectUdpConnectRemoteSend<T>
where
    T: AsyncUdpSend,
{
    fn buf_reserve_length(&self) -> usize {
        0
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
    ) -> Poll<Result<usize, UdpCopyRemoteError>> {
        let nw = ready!(self.inner.poll_send(cx, &buf[buf_off..buf_len]))
            .map_err(UdpCopyRemoteError::SendFailed)?;
        Poll::Ready(Ok(nw))
    }
}
