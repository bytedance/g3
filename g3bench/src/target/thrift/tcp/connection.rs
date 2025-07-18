/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use tokio::net::tcp;

use g3_io_ext::{LimitedReader, LimitedWriter};

pub(super) struct ThriftConnection {
    pub(super) reader: LimitedReader<tcp::OwnedReadHalf>,
    pub(super) writer: LimitedWriter<tcp::OwnedWriteHalf>,
}

impl ThriftConnection {
    pub(super) fn new(
        reader: LimitedReader<tcp::OwnedReadHalf>,
        writer: LimitedWriter<tcp::OwnedWriteHalf>,
    ) -> Self {
        ThriftConnection { reader, writer }
    }
}
