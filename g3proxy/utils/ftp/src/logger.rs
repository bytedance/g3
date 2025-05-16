/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
