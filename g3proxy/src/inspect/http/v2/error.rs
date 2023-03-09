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
use std::time::Duration;

use anyhow::anyhow;
use http::{Response, StatusCode, Version};
use thiserror::Error;

use g3_h2::H2StreamBodyTransferError;
use g3_icap_client::reqmod::h2::H2ReqmodAdaptationError;
use g3_icap_client::respmod::h2::H2RespmodAdaptationError;
use g3_io_ext::IdleForceQuitReason;

#[derive(Debug, Error)]
pub(crate) enum H2InterceptionError {
    #[error("upstream io error during handshake: {0:?}")]
    UpstreamHandshakeIoError(io::Error),
    #[error("timeout to handshake with upstream")]
    UpstreamHandshakeTimeout,
    #[error("client io error during handshake: {0:?}")]
    ClientHandshakeIoError(io::Error),
    #[error("timeout to handshake with client")]
    ClientHandshakeTimeout,
    #[error("upstream connection closed: {0}")]
    UpstreamConnectionClosed(h2::Error),
    #[error("upstream connection disconnected")]
    UpstreamConnectionDisconnected,
    #[error("upstream connection finished")]
    UpstreamConnectionFinished,
    #[error("client connection closed: {0}")]
    ClientConnectionClosed(h2::Error),
    #[error("client connection disconnected")]
    ClientConnectionDisconnected,
    #[error("client connection finished")]
    ClientConnectionFinished,
    #[error("canceled as user blocked")]
    CanceledAsUserBlocked,
    #[error("canceled as server quit")]
    CanceledAsServerQuit,
    #[error("idle after {0:?} x {1}")]
    Idle(Duration, i32),
    #[error("unexpected error: {0:}")]
    UnexpectedError(anyhow::Error),
}

impl H2InterceptionError {
    pub(super) fn upstream_handshake_failed(e: h2::Error) -> Self {
        if e.is_io() {
            H2InterceptionError::UpstreamHandshakeIoError(e.into_io().unwrap())
        } else {
            H2InterceptionError::UnexpectedError(anyhow!(
                "unhandled error while handshake to upstream: {e:?}"
            ))
        }
    }

    pub(super) fn client_handshake_failed(e: h2::Error) -> Self {
        if e.is_io() {
            H2InterceptionError::ClientHandshakeIoError(e.into_io().unwrap())
        } else {
            H2InterceptionError::UnexpectedError(anyhow!(
                "unhandled error while handshake to client: {e:?}"
            ))
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum H2StreamTransferError {
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
    #[error("internal adapter error: {0}")]
    InternalAdapterError(anyhow::Error),
    #[error("failed to open upstream stream: {0}")]
    UpstreamStreamOpenFailed(h2::Error),
    #[error("timeout to open upstream stream")]
    UpstreamStreamOpenTimeout,
    #[error("failed to send request head: {0}")]
    RequestHeadSendFailed(h2::Error),
    #[error("invalid Host header")]
    InvalidHostHeader,
    #[error("failed to recv response head: {0}")]
    ResponseHeadRecvFailed(h2::Error),
    #[error("timeout to recv response head")]
    ResponseHeadRecvTimeout,
    #[error("failed to send response head: {0}")]
    ResponseHeadSendFailed(h2::Error),
    #[error("failed to transfer request body: {0}")]
    RequestBodyTransferFailed(H2StreamBodyTransferError),
    #[error("failed to transfer response body: {0}")]
    ResponseBodyTransferFailed(H2StreamBodyTransferError),
    #[error("canceled as user blocked")]
    CanceledAsUserBlocked,
    #[error("canceled as server quit")]
    CanceledAsServerQuit,
    #[error("read from http client idle")]
    HttpClientReadIdle,
    #[error("write to http client idle")]
    HttpClientWriteIdle,
    #[error("read from http upstream idle")]
    HttpUpstreamReadIdle,
    #[error("write to http upstream idle")]
    HttpUpstreamWriteIdle,
    #[error("idle after {0:?} x {1}")]
    Idle(Duration, i32),
    #[error("push wait error: {0}")]
    PushWaitError(h2::Error),
}

impl H2StreamTransferError {
    pub(super) fn build_reply(&self) -> Option<Response<()>> {
        let status_code = match self {
            H2StreamTransferError::UpstreamStreamOpenFailed(_)
            | H2StreamTransferError::UpstreamStreamOpenTimeout => {
                // we should refuse stream
                return None;
            }
            H2StreamTransferError::RequestHeadSendFailed(_) => StatusCode::BAD_GATEWAY,
            H2StreamTransferError::InvalidHostHeader => StatusCode::BAD_REQUEST,
            H2StreamTransferError::ResponseHeadRecvFailed(_) => StatusCode::BAD_GATEWAY,
            H2StreamTransferError::ResponseHeadRecvTimeout => StatusCode::GATEWAY_TIMEOUT,
            _ => return None,
        };
        let rsp = Response::builder()
            .status(status_code)
            .version(Version::HTTP_2);
        rsp.body(()).ok()
    }
}

impl From<H2ReqmodAdaptationError> for H2StreamTransferError {
    fn from(e: H2ReqmodAdaptationError) -> Self {
        match e {
            H2ReqmodAdaptationError::InternalServerError(s) => {
                H2StreamTransferError::InternalServerError(s)
            }
            H2ReqmodAdaptationError::HttpClientRecvDataFailed(e) => {
                H2StreamTransferError::RequestBodyTransferFailed(
                    H2StreamBodyTransferError::RecvDataFailed(e),
                )
            }
            H2ReqmodAdaptationError::HttpClientRecvTrailerFailed(e) => {
                H2StreamTransferError::RequestBodyTransferFailed(
                    H2StreamBodyTransferError::RecvTrailersFailed(e),
                )
            }
            H2ReqmodAdaptationError::HttpUpstreamSendHeadFailed(e) => {
                H2StreamTransferError::RequestHeadSendFailed(e)
            }
            H2ReqmodAdaptationError::HttpUpstreamSendDataFailed(e) => {
                H2StreamTransferError::RequestBodyTransferFailed(
                    H2StreamBodyTransferError::SendDataFailed(e),
                )
            }
            H2ReqmodAdaptationError::HttpUpstreamSendTrailedFailed(e) => {
                H2StreamTransferError::RequestBodyTransferFailed(
                    H2StreamBodyTransferError::SendTrailersFailed(e),
                )
            }
            H2ReqmodAdaptationError::HttpClientReadIdle => {
                H2StreamTransferError::HttpClientReadIdle
            }
            H2ReqmodAdaptationError::HttpUpstreamWriteIdle => {
                H2StreamTransferError::HttpUpstreamWriteIdle
            }
            H2ReqmodAdaptationError::IdleForceQuit(reason) => match reason {
                IdleForceQuitReason::UserBlocked => H2StreamTransferError::CanceledAsUserBlocked,
                IdleForceQuitReason::ServerQuit => H2StreamTransferError::CanceledAsServerQuit,
            },
            H2ReqmodAdaptationError::HttpUpstreamRecvResponseFailed(e) => {
                H2StreamTransferError::ResponseHeadRecvFailed(e)
            }
            H2ReqmodAdaptationError::HttpUpstreamRecvResponseTimeout => {
                H2StreamTransferError::ResponseHeadRecvTimeout
            }
            e => H2StreamTransferError::InternalAdapterError(anyhow!("reqmod: {e}")),
        }
    }
}

impl From<H2RespmodAdaptationError> for H2StreamTransferError {
    fn from(e: H2RespmodAdaptationError) -> Self {
        match e {
            H2RespmodAdaptationError::InternalServerError(s) => {
                H2StreamTransferError::InternalServerError(s)
            }
            H2RespmodAdaptationError::HttpUpstreamRecvDataFailed(e) => {
                H2StreamTransferError::ResponseBodyTransferFailed(
                    H2StreamBodyTransferError::RecvDataFailed(e),
                )
            }
            H2RespmodAdaptationError::HttpUpstreamRecvTrailerFailed(e) => {
                H2StreamTransferError::ResponseBodyTransferFailed(
                    H2StreamBodyTransferError::RecvTrailersFailed(e),
                )
            }
            H2RespmodAdaptationError::HttpClientSendHeadFailed(e) => {
                H2StreamTransferError::ResponseHeadSendFailed(e)
            }
            H2RespmodAdaptationError::HttpClientSendDataFailed(e) => {
                H2StreamTransferError::ResponseBodyTransferFailed(
                    H2StreamBodyTransferError::SendDataFailed(e),
                )
            }
            H2RespmodAdaptationError::HttpClientSendTrailerFailed(e) => {
                H2StreamTransferError::ResponseBodyTransferFailed(
                    H2StreamBodyTransferError::SendTrailersFailed(e),
                )
            }
            H2RespmodAdaptationError::HttpUpstreamReadIdle => {
                H2StreamTransferError::HttpUpstreamReadIdle
            }
            H2RespmodAdaptationError::HttpClientWriteIdle => {
                H2StreamTransferError::HttpClientWriteIdle
            }
            H2RespmodAdaptationError::IdleForceQuit(reason) => match reason {
                IdleForceQuitReason::UserBlocked => H2StreamTransferError::CanceledAsUserBlocked,
                IdleForceQuitReason::ServerQuit => H2StreamTransferError::CanceledAsServerQuit,
            },
            e => H2StreamTransferError::InternalAdapterError(anyhow!("respmod: {e}")),
        }
    }
}
