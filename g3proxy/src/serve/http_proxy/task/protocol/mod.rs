/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_io_ext::{LimitedBufReader, LimitedWriter};

mod request;
pub(super) use request::HttpProxyRequest;

pub(super) type HttpClientReader<CDR> = LimitedBufReader<CDR>;
pub(super) type HttpClientWriter<CDW> = LimitedWriter<CDW>;
