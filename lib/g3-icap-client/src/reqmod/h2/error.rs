/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use g3_http::client::HttpResponseParseError;
use g3_http::server::HttpRequestParseError;
use g3_io_ext::IdleForceQuitReason;

use crate::reason::IcapErrorReason;
use crate::reqmod::IcapReqmodParseError;

#[derive(Debug, Error)]
pub enum H2ReqmodAdaptationError {
    #[error("write to icap server failed: {0:?}")]
    IcapServerWriteFailed(io::Error),
    #[error("read from icap server failed: {0:?}")]
    IcapServerReadFailed(io::Error),
    #[error("connection closed by icap server")]
    IcapServerConnectionClosed,
    #[error("invalid response from icap server: {0}")]
    InvalidIcapServerResponse(#[from] IcapReqmodParseError),
    #[error("invalid http error response from icap server: {0}")]
    InvalidIcapServerHttpResponse(#[from] HttpResponseParseError),
    #[error("invalid http request from icap server: {0}")]
    InvalidIcapServerHttpRequest(#[from] HttpRequestParseError),
    #[error("error response from icap server: {0} ({1} {2})")]
    IcapServerErrorResponse(IcapErrorReason, u16, String),
    #[error("recv data from http client failed: {0}")]
    HttpClientRecvDataFailed(h2::Error),
    #[error("recv trailer from http client failed: {0}")]
    HttpClientRecvTrailerFailed(h2::Error),
    #[error("send head to http upstream failed: {0}")]
    HttpUpstreamSendHeadFailed(h2::Error),
    #[error("upstream not in send state")]
    HttpUpstreamNotInSendState,
    #[error("send data to http upstream failed: {0}")]
    HttpUpstreamSendDataFailed(h2::Error),
    #[error("send trailer to http upstream failed: {0}")]
    HttpUpstreamSendTrailedFailed(h2::Error),
    #[error("recv response from http upstream failed: {0}")]
    HttpUpstreamRecvResponseFailed(h2::Error),
    #[error("recv response from http upstream timeout")]
    HttpUpstreamRecvResponseTimeout,
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
    #[error("force quit from idle checker: {0:?}")]
    IdleForceQuit(IdleForceQuitReason),
    #[error("idle while reading from http client")]
    HttpClientReadIdle,
    #[error("idle while writing to http upstream")]
    HttpUpstreamWriteIdle,
    #[error("idle while reading from icap server")]
    IcapServerReadIdle,
    #[error("idle while writing to icap server")]
    IcapServerWriteIdle,
    #[error("not implemented feature: {0}")]
    NotImplemented(&'static str),
}
