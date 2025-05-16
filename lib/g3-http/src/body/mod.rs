/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpBodyType {
    ContentLength(u64),
    Chunked,
    ReadUntilEnd,
}

mod reader;
pub use reader::HttpBodyReader;

mod decoder;
pub use decoder::HttpBodyDecodeReader;

mod preview;
pub use preview::{PreviewData, PreviewDataState, PreviewError};

mod body_to_chunked;
pub use body_to_chunked::H1BodyToChunkedTransfer;

mod stream_to_chunked;
pub use stream_to_chunked::StreamToChunkedTransfer;

mod chunked_decoder;
pub use chunked_decoder::ChunkedDataDecodeReader;

mod trailer_reader;
pub use trailer_reader::{TrailerReadError, TrailerReader};
