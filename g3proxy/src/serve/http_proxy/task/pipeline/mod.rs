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

use super::{
    CommonTaskContext, FtpOverHttpTask, HttpProxyConnectTask, HttpProxyForwardTask,
    HttpProxyServerStats, HttpProxyUntrustedTask, protocol,
};

mod reader;
mod writer;

pub(crate) use reader::HttpProxyPipelineReaderTask;
pub(crate) use writer::HttpProxyPipelineWriterTask;

mod stats;
use stats::HttpProxyCltWrapperStats;
pub(crate) use stats::HttpProxyPipelineStats;
