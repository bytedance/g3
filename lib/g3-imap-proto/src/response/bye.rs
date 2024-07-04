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

use std::io;

use tokio::io::AsyncWrite;

use g3_io_ext::LimitedWriteExt;

const BYE_BLOCKED: &str = "* BYE Blocked; connection not allowed\r\n";
const BYE_AUTO_LOGOUT: &str = "* BYE Autologout; idle for too long\r\n";
const BYE_INTERNAL_ERROR: &str = "* BYE PanicShutdown; shutdown due to internal error\r\n";

pub struct ByeResponse {}

impl ByeResponse {
    pub async fn reply_blocked<W>(writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all_flush(BYE_BLOCKED.as_bytes()).await
    }

    pub async fn reply_idle_logout<W>(writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all_flush(BYE_AUTO_LOGOUT.as_bytes()).await
    }

    pub async fn reply_internal_error<W>(writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all_flush(BYE_INTERNAL_ERROR.as_bytes()).await
    }
}
