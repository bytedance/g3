/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

impl StdLogValue {
    fn message_str(&self) -> &str {
        if self.message.is_empty() {
            "()"
        } else {
            &self.message
        }
    }
}

pub fn new_async_logger(
    async_conf: &AsyncLogConfig,
    append_code_position: bool,
    use_stdout: bool,
) -> AsyncLogger<StdLogValue, StdLogFormatter> {
    let (sender, receiver) = flume::bounded::<StdLogValue>(async_conf.channel_capacity);

    let stats = Arc::new(LogStats::default());

    let io_thread = AsyncIoThread {
        receiver,
        stats: Arc::clone(&stats),
    };

    let _detached_thread = std::thread::Builder::new()
        .name(async_conf.thread_name.clone())
        .spawn(move || {
            if use_stdout {
                io_thread.run_with_stdout();
            } else {
                io_thread.run_with_stderr();
            }
        });

    AsyncLogger::new(sender, StdLogFormatter::new(append_code_position), stats)
}

struct AsyncIoThread {
    receiver: Receiver<StdLogValue>,
    stats: Arc<LogStats>,
}

impl AsyncIoThread {
    fn write_time<IO: Write>(&self, io: &mut IO) -> io::Result<()> {
        let datetime = Local::now();
        let fmt = datetime.format_with_items(g3_datetime::format::log::STDIO.iter());
        write!(io, "{fmt}")?;
        Ok(())
    }

    fn run_with_stderr(self) {
        let stderr = io::stderr();
        if stderr.is_terminal() {
            self.run_console(stderr)
        } else {
            self.run_plain(stderr)
        }
    }

    fn run_with_stdout(self) {
        let stdout = io::stdout();
        if stdout.is_terminal() {
            self.run_console(stdout)
        } else {
            self.run_plain(stdout)
        }
    }

    fn run_plain<IO: Write>(&self, mut io: IO) {
        let mut buf: Vec<u8> = Vec::with_capacity(1024);
        while let Ok(v) = self.receiver.recv() {
            buf.clear();
            let _ = self.write_plain(&mut buf, v);
            self.write_buf(&mut io, &buf);

            while let Ok(v) = self.receiver.try_recv() {
                buf.clear();
                let _ = self.write_plain(&mut buf, v);
                self.write_buf(&mut io, &buf);
            }

            let _ = io.flush();
        }
    }

    fn write_plain<IO: Write>(&self, io: &mut IO, v: StdLogValue) -> io::Result<()> {
        self.write_time(io)?;
        write!(io, " {}", v.level)?;
        for (k, v) in &v.kv_pairs {
            write!(io, " {k}: {v},")?;
        }
        write!(io, " {}", v.message_str())?;
        if let Some(location) = v.location {
            write!(io, " <{location}>")?;
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
            self.write_buf(&mut io, &buf);

            while let Ok(v) = self.receiver.try_recv() {
                buf.clear();
                let _ = self.write_console(&mut buf, v);
                self.write_buf(&mut io, &buf);
            }

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
            " {}{}{}",
            level_color.render(),
            v.level,
            level_color.render_reset(),
        )?;

        for (k, v) in &v.kv_pairs {
            write!(io, " {bold_s}{k}{bold_e}={v},")?;
        }

        write!(io, " {bold_s}{}{bold_e}", v.message_str())?;

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

    fn write_buf<IO: Write>(&self, io: &mut IO, buf: &[u8]) {
        match io.write_all(buf) {
            Ok(_) => {
                self.stats.io.add_passed();
                self.stats.io.add_size(buf.len());
            }
            Err(_) => self.stats.drop.add_peer_unreachable(),
        }
    }
}
