/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use g3_http::client::HttpResponseParseError;
use g3_io_ext::IdleForceQuitReason;

use crate::reason::IcapErrorReason;
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
    #[error("invalid http body from icap server: {0:?}")]
    InvalidHttpBodyFromIcapServer(anyhow::Error),
    #[error("error response from icap server: {0} ({1} {2})")]
    IcapServerErrorResponse(IcapErrorReason, u16, String),
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
