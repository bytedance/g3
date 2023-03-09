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

use std::io::{self, Write};
use std::sync::Arc;

use chrono::Local;
use flume::Receiver;
use slog::Level;

use g3_types::log::{AsyncLogConfig, AsyncLogger, LogStats};

#[macro_use]
mod macros;

mod format;
use format::StdLogFormatter;

pub struct StdLogValue {
    level: Level,
    message: String,
    kv_pairs: Vec<(String, String)>,
    location: Option<String>,
}

pub fn new_async_logger(
    async_conf: &AsyncLogConfig,
    append_code_position: bool,
) -> AsyncLogger<StdLogValue, StdLogFormatter> {
    let (sender, receiver) = flume::bounded::<StdLogValue>(async_conf.channel_capacity);

    let stats = Arc::new(LogStats::default());

    let io_thread = AsyncIoThread { receiver };

    let _detached_thread = std::thread::Builder::new()
        .name(async_conf.thread_name.clone())
        .spawn(move || {
            io_thread.run_to_end();
        });

    AsyncLogger::new(sender, StdLogFormatter::new(append_code_position), stats)
}

struct AsyncIoThread {
    receiver: Receiver<StdLogValue>,
}

impl AsyncIoThread {
    fn write_time<IO: Write>(&self, io: &mut IO) -> io::Result<()> {
        let datetime = Local::now();
        let fmt = datetime.format_with_items(g3_datetime::format::log::STDIO.iter());
        write!(io, "{fmt}")?;
        Ok(())
    }

    fn run_to_end(self) {
        if console::user_attended_stderr() {
            self.run_console(std::io::stderr())
        } else {
            self.run_plain(std::io::stderr())
        }
    }

    fn run_plain<IO: Write>(&self, mut io: IO) {
        let mut buf: Vec<u8> = Vec::with_capacity(1024);
        while let Ok(v) = self.receiver.recv() {
            buf.clear();
            let _ = self.write_plain(&mut buf, v);
            let _ = io.write(&buf);
            let _ = io.flush();
        }
    }

    fn write_plain<IO: Write>(&self, io: &mut IO, v: StdLogValue) -> io::Result<()> {
        self.write_time(io)?;
        write!(io, " {} {}", v.level, v.message)?;
        if let Some(location) = v.location {
            write!(io, " <{location}>")?;
        }
        for (k, v) in v.kv_pairs {
            write!(io, ", {k}: {v}")?;
        }
        writeln!(io)?;
        io.flush()?;
        Ok(())
    }

    fn run_console<IO: Write>(&self, mut io: IO) {
        let mut buf: Vec<u8> = Vec::with_capacity(1024);
        while let Ok(v) = self.receiver.recv() {
            buf.clear();
            let _ = self.write_console(&mut buf, v);
            let _ = io.write(&buf);
            let _ = io.flush();
        }
    }

    fn write_console<IO: Write>(&self, io: &mut IO, v: StdLogValue) -> io::Result<()> {
        use console::{style, Style};

        self.write_time(io)?;
        let level_color = match v.level {
            Level::Critical => Style::new().magenta(),
            Level::Error => Style::new().red(),
            Level::Warning => Style::new().yellow(),
            Level::Info => Style::new().green(),
            Level::Debug => Style::new().cyan(),
            Level::Trace => Style::new().blue(),
        };
        write!(
            io,
            " {} {}",
            level_color.apply_to(v.level),
            style(v.message).bold()
        )?;

        for (k, v) in v.kv_pairs {
            write!(io, ", {}={}", style(k).bold(), v)?;
        }

        if let Some(location) = v.location {
            write!(io, " <{}>", style(location).italic())?;
        }
        writeln!(io)?;
        io.flush()?;

        Ok(())
    }
}
