/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use g3_http::client::HttpResponseParseError;
use g3_http::server::HttpRequestParseError;
use g3_io_ext::IdleForceQuitReason;

use crate::reqmod::IcapReqmodParseError;

#[derive(Debug, Error)]
pub enum SmtpAdaptationError {
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
    #[error("error response from icap server: {0} {1}")]
    IcapServerErrorResponse(u16, String),
    #[error("read from smtp client failed: {0:?}")]
    SmtpClientReadFailed(io::Error),
    #[error("invalid smtp client message")]
    InvalidSmtpClientMessage,
    #[error("write to smtp upstream failed: {0:?}")]
    SmtpUpstreamWriteFailed(io::Error),
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
    #[error("force quit from idle checker: {0:?}")]
    IdleForceQuit(IdleForceQuitReason),
    #[error("idle while reading from smtp client")]
    SmtpClientReadIdle,
    #[error("idle while writing to smtp upstream")]
    SmtpUpstreamWriteIdle,
    #[error("idle while reading from icap server")]
    IcapServerReadIdle,
    #[error("idle while writing to icap server")]
    IcapServerWriteIdle,
    #[error("not implemented feature: {0}")]
    NotImplemented(&'static str),
}
