/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::panic::{RefUnwindSafe, UnwindSafe};

use slog::{OwnedKVList, Record};

use super::SyslogHeader;

#[macro_use]
mod macros;
mod serde;

mod cee;
mod rfc3164;
mod rfc5424;

pub(super) use cee::{CEE_EVENT_FLAG, FormatterRfc3164Cee, FormatterRfc5424Cee};
pub(super) use rfc3164::FormatterRfc3164;
pub(super) use rfc5424::FormatterRfc5424;

#[cfg(feature = "yaml")]
mod yaml;

pub trait SyslogFormatter {
    fn append_report_ts(&mut self, enable: bool);

    fn format_slog(
        &self,
        w: &mut Vec<u8>,
        header: &SyslogHeader,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> Result<(), slog::Error>;
}

pub type BoxSyslogFormatter = Box<dyn SyslogFormatter + Send + Sync + UnwindSafe + RefUnwindSafe>;

#[derive(Clone, Debug)]
pub enum SyslogFormatterKind {
    Rfc3164,
    /// rfc3164 cee formatter with event flag
    Rfc3164Cee(String),
    /// rfc5424 formatter with enterprise id and optional message id
    Rfc5424(i32, Option<String>),
    /// rfc5424 cee formatter with optional message id and event flag
    Rfc5424Cee(Option<String>, String),
}
