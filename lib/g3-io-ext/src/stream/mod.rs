/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod stream_type;
pub use stream_type::AsyncStream;

mod limited;
pub use limited::*;

mod copy;
pub use copy::{ROwnedStreamCopy, StreamCopy, StreamCopyConfig, StreamCopyError};

mod buf;
pub use buf::{BufReadCopy, FlexBufReader, LimitedBufReader, OnceBufReader};

mod line_recv_buf;
pub use line_recv_buf::{LineRecvBuf, RecvLineError};

mod line_recv_vec;
pub use line_recv_vec::LineRecvVec;

mod ext;
pub use ext::{LimitedBufReadExt, LimitedReadExt, LimitedWriteExt};

#[cfg(feature = "openssl")]
pub mod openssl;

#[cfg(feature = "rustls")]
pub mod rustls;
