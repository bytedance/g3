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

use std::io;

use thiserror::Error;

use g3_http::client::HttpResponseParseError;
use g3_http::PreviewError;
use g3_io_ext::IdleForceQuitReason;

use crate::respmod::IcapRespmodParseError;

#[derive(Debug, Error)]
pub enum H1RespmodAdaptationError {
    #[error("write to icap server failed: {0:?}")]
    IcapServerWriteFailed(io::Error),
    #[error("read from icap server failed: {0:?}")]
    IcapServerReadFailed(io::Error),
    #[error("connection closed by icap server")]
    IcapServerConnectionClosed,
    #[error("invalid response from icap server: {0}")]
    InvalidIcapServerResponse(#[from] IcapRespmodParseError),
    #[error("invalid http error response from icap server: {0}")]
    InvalidIcapServerHttpResponse(#[from] HttpResponseParseError),
    #[error("error response from icap server: {0} {1}")]
    IcapServerErrorResponse(u16, String),
    #[error("read from http upstream failed: {0:?}")]
    HttpUpstreamReadFailed(io::Error),
    #[error("invalid body in http upstream response")]
    InvalidHttpUpstreamResponseBody,
    #[error("write to http client failed: {0:?}")]
    HttpClientWriteFailed(io::Error),
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
    #[error("force quit from idle checker: {0:?}")]
    IdleForceQuit(IdleForceQuitReason),
    #[error("idle while reading from http upstream")]
    HttpUpstreamReadIdle,
    #[error("idle while writing to http client")]
    HttpClientWriteIdle,
    #[error("idle while reading from icap server")]
    IcapServerReadIdle,
    #[error("idle while writing to icap server")]
    IcapServerWriteIdle,
    #[error("not implemented feature: {0}")]
    NotImplemented(&'static str),
}

impl From<PreviewError> for H1RespmodAdaptationError {
    fn from(e: PreviewError) -> Self {
        match e {
            PreviewError::ReadError(e) => H1RespmodAdaptationError::HttpUpstreamReadFailed(e),
            PreviewError::ReaderClosed => {
                H1RespmodAdaptationError::HttpUpstreamReadFailed(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed while reading preview data",
                ))
            }
            PreviewError::AlreadyPolled => {
                H1RespmodAdaptationError::InternalServerError("preview data already polled")
            }
            PreviewError::InvalidChunkedBody => {
                H1RespmodAdaptationError::InvalidHttpUpstreamResponseBody
            }
        }
    }
}
