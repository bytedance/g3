/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpProxySubProtocol {
    TcpConnect,
    HttpForward,
    HttpsForward,
    FtpOverHttp,
}
