/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::io::BufReader;

use g3_io_ext::{LimitedReader, LimitedWriter};

use crate::module::http::{BoxHttpForwardReader, BoxHttpForwardWriter};

pub(super) struct SavedHttpForwardConnection {
    pub(super) reader: BufReader<LimitedReader<BoxHttpForwardReader>>,
    pub(super) writer: LimitedWriter<BoxHttpForwardWriter>,
}

impl SavedHttpForwardConnection {
    pub(super) fn new(
        reader: BufReader<LimitedReader<BoxHttpForwardReader>>,
        writer: LimitedWriter<BoxHttpForwardWriter>,
    ) -> Self {
        SavedHttpForwardConnection { reader, writer }
    }
}
