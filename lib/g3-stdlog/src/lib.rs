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

use std::io::{self, IsTerminal, Write};
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
        let stderr = io::stderr();
        if stderr.is_terminal() {
            self.run_console(stderr)
        } else {
            self.run_plain(stderr)
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
        use anstyle::{AnsiColor, Color, Style};

        const COLOR_MAGENTA: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Magenta)));
        const COLOR_RED: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));
        const COLOR_YELLOW: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
        const COLOR_GREEN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
        const COLOR_CYAN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
        const COLOR_BLUE: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Blue)));
        const STYLE_BOLD: Style = Style::new().bold();
        const STYLE_ITALIC: Style = Style::new().italic();

        let bold_s = STYLE_BOLD.render();
        let bold_e = STYLE_BOLD.render_reset();

        self.write_time(io)?;
        let level_color = match v.level {
            Level::Critical => COLOR_MAGENTA,
            Level::Error => COLOR_RED,
            Level::Warning => COLOR_YELLOW,
            Level::Info => COLOR_GREEN,
            Level::Debug => COLOR_CYAN,
            Level::Trace => COLOR_BLUE,
        };
        write!(
            io,
            " {}{}{} {bold_s}{}{bold_e}",
            level_color.render(),
            v.level,
            level_color.render_reset(),
            v.message,
        )?;

        for (k, v) in v.kv_pairs {
            write!(io, ", {bold_s}{k}{bold_e}={v}")?;
        }

        if let Some(location) = v.location {
            write!(
                io,
                " <{}{location}{}>",
                STYLE_ITALIC.render(),
                STYLE_ITALIC.render_reset()
            )?;
        }
        writeln!(io)?;
        io.flush()?;

        Ok(())
    }
}
