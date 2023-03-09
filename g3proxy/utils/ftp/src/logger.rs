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

use log::{Level, LevelFilter, Log, Metadata, Record};

use g3_ftp_client::FTP_DEBUG_LOG_TARGET;

pub(crate) struct SyncLogger {
    verbose_level: u8,
}

impl SyncLogger {
    pub(crate) fn new(verbose_level: u8) -> Self {
        SyncLogger { verbose_level }
    }

    pub(crate) fn into_global_logger(self) -> Result<(), log::SetLoggerError> {
        log::set_boxed_logger(Box::new(self))?;
        log::set_max_level(LevelFilter::Debug);
        Ok(())
    }
}

impl Log for SyncLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        match metadata.target() {
            FTP_DEBUG_LOG_TARGET => match metadata.level() {
                Level::Trace => false,
                Level::Debug => self.verbose_level > 0,
                _ => true,
            },
            _ => false,
        }
    }

    #[allow(clippy::single_match)]
    fn log(&self, record: &Record) {
        match record.target() {
            FTP_DEBUG_LOG_TARGET => match record.level() {
                Level::Trace => {}
                Level::Debug => {
                    if self.verbose_level > 0 {
                        eprintln!("{}", record.args());
                    }
                }
                _ => {
                    eprintln!("{}", record.args())
                }
            },
            _ => {}
        }
    }

    fn flush(&self) {}
}
