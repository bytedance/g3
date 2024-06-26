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
