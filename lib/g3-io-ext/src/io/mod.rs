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
