/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;

use tokio::io::AsyncWrite;

use g3_io_ext::LimitedWriteExt;

pub struct BadResponse {}

impl BadResponse {
    pub async fn reply_invalid_command<W>(writer: &mut W, tag: &str) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let message = format!("{tag} BAD invalid command\r\n");
        writer.write_all_flush(message.as_bytes()).await
    }

    pub async fn reply_append_blocked<W>(writer: &mut W, tag: &str) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let message = format!("{tag} BAD the message is blocked\r\n");
        writer.write_all_flush(message.as_bytes()).await
    }
}
