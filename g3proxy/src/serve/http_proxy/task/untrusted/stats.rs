/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{ArcLimitedReaderStats, LimitedReaderStats};

use super::HttpProxyServerStats;

pub(super) struct UntrustedCltReadWrapperStats {
    server: Arc<HttpProxyServerStats>,
}

impl UntrustedCltReadWrapperStats {
    pub(super) fn new_obj(server: &Arc<HttpProxyServerStats>) -> ArcLimitedReaderStats {
        let stats = UntrustedCltReadWrapperStats {
            server: Arc::clone(server),
        };
        Arc::new(stats)
    }
}

impl LimitedReaderStats for UntrustedCltReadWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        self.server.io_untrusted.add_in_bytes(size as u64);
    }
}
