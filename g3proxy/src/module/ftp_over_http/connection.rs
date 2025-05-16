/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use g3_io_ext::LimitedStream;

pub(crate) trait FtpRemoteConnection: AsyncRead + AsyncWrite {}

impl FtpRemoteConnection for TcpStream {}

impl<S> FtpRemoteConnection for LimitedStream<S> where S: AsyncRead + AsyncWrite {}

pub(crate) type BoxFtpRemoteConnection = Box<dyn FtpRemoteConnection + Send + Unpin>;
