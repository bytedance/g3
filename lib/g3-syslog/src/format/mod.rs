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

use std::panic::{RefUnwindSafe, UnwindSafe};

use slog::{OwnedKVList, Record};

use super::SyslogHeader;

#[macro_use]
mod macros;
mod serde;

mod cee;
mod rfc3164;
mod rfc5424;

pub(super) use cee::{FormatterRfc3164Cee, FormatterRfc5424Cee, CEE_EVENT_FLAG};
pub(super) use rfc3164::FormatterRfc3164;
pub(super) use rfc5424::FormatterRfc5424;

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
