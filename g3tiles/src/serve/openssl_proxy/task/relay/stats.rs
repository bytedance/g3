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

use std::sync::Arc;

use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use crate::serve::openssl_proxy::OpensslProxyServerStats;

#[derive(Clone)]
pub(crate) struct OpensslRelayTaskCltWrapperStats {
    server: Arc<OpensslProxyServerStats>,
    task: Arc<TcpStreamTaskStats>,
}

impl OpensslRelayTaskCltWrapperStats {
    pub(crate) fn new(
        server: &Arc<OpensslProxyServerStats>,
        task: &Arc<TcpStreamTaskStats>,
    ) -> Self {
        OpensslRelayTaskCltWrapperStats {
            server: Arc::clone(server),
            task: Arc::clone(task),
        }
    }
}

impl LimitedReaderStats for OpensslRelayTaskCltWrapperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.read.add_bytes(size);
        self.server.add_read(size);
    }
}

impl LimitedWriterStats for OpensslRelayTaskCltWrapperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.task.clt.write.add_bytes(size);
        self.server.add_write(size);
    }
}
