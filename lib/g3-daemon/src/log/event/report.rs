/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicUsize, Ordering};

use log::{info, warn};
use slog::{Drain, Level, Never, OwnedKVList, Record};

/// report log error to process log
pub struct ReportLogIoError<D: Drain<Err = slog::Error, Ok = ()>> {
    logger_id: String,
    error_count: AtomicUsize,
    report_mask: usize,
    inner: D,
}

impl<D: Drain<Err = slog::Error, Ok = ()>> ReportLogIoError<D> {
    pub fn new(drain: D, logger_name: &str, error_report_mask: usize) -> Self {
        ReportLogIoError {
            logger_id: logger_name.to_string(),
            error_count: AtomicUsize::new(0),
            report_mask: error_report_mask,
            inner: drain,
        }
    }
}

impl<D: Drain<Err = slog::Error, Ok = ()>> Drain for ReportLogIoError<D> {
    type Ok = ();
    type Err = Never;

    fn log(&self, record: &Record, logger_values: &OwnedKVList) -> Result<(), Never> {
        match self.inner.log(record, logger_values) {
            Ok(_) => {
                let error_count = self.error_count.swap(0, Ordering::Relaxed);
                if error_count != 0 {
                    info!(
                        "logger {} back to work, lost {error_count} messages",
                        self.logger_id
                    );
                }
            }
            Err(e) => {
                let old_count = self.error_count.fetch_add(1, Ordering::Relaxed);
                match old_count {
                    0 | 1 => warn!("logger {} got io error: {e:?}", self.logger_id),
                    _ => {
                        if (old_count & self.report_mask) == 0 {
                            warn!(
                                "logger {} has seen {old_count} errors, latest io error: {e:?}",
                                self.logger_id
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        self.inner.is_enabled(level)
    }
}
