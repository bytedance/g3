/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod limited_copy;
mod limited_read;
mod limited_stream;
mod limited_write;

pub use limited_copy::{LimitedCopy, LimitedCopyConfig, LimitedCopyError, ROwnedLimitedCopy};
pub use limited_read::{
    ArcLimitedReaderStats, LimitedReader, LimitedReaderStats, NilLimitedReaderStats, SizedReader,
};
pub use limited_stream::LimitedStream;
pub use limited_write::{
    ArcLimitedWriterStats, LimitedWriter, LimitedWriterStats, NilLimitedWriterStats,
};

mod buf;
pub use buf::{FlexBufReader, LimitedBufCopy, LimitedBufReader, OnceBufReader};

mod line_recv_buf;
pub use line_recv_buf::{LineRecvBuf, RecvLineError};

mod line_recv_vec;
pub use line_recv_vec::LineRecvVec;

mod ext;
pub use ext::{LimitedBufReadExt, LimitedReadExt, LimitedWriteExt};

pub(super) mod stream;
pub use stream::AsyncStream;
