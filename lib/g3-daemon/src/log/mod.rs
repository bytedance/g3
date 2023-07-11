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

pub mod process;

mod report;
pub use report::ReportLogIoError;

mod stats;
use stats::LoggerStats;

pub mod metric;

mod registry;

mod runtime;
pub use runtime::{create_logger, create_shared_logger};

mod config;
pub use config::{LogConfig, LogConfigDriver};

pub struct LogConfigContainer {
    inner: Option<LogConfig>,
}

impl LogConfigContainer {
    pub const fn new() -> Self {
        LogConfigContainer { inner: None }
    }

    pub fn set_default(&mut self, config: LogConfig) {
        if self.inner.is_none() {
            self.set(config);
        }
    }

    pub fn set(&mut self, config: LogConfig) {
        self.inner = Some(config)
    }

    pub fn get(&self, program_name: &'static str) -> LogConfig {
        if let Some(config) = &self.inner {
            config.clone()
        } else {
            LogConfig::default_discard(program_name)
        }
    }
}
