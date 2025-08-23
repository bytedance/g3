/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicBool, Ordering};

use log::warn;

use crate::opts::DaemonArgs;

static TRIGGER_PANIC_QUIT: AtomicBool = AtomicBool::new(true);

pub fn set_hook(args: &DaemonArgs) {
    if !args.panic_quit {
        return;
    }

    let monitored = args.monitored;
    std::panic::set_hook(Box::new(move |panic_info| {
        let panic_message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "-"
        };

        match std::thread::current().name() {
            Some(thread) => {
                if let Some(location) = panic_info.location() {
                    warn!(
                        "thread '{thread}' panicked at {}:{}:{}: {panic_message}",
                        location.file(),
                        location.line(),
                        location.column()
                    );
                } else {
                    warn!("thread '{thread}' panicked: {panic_message}");
                }
            }
            None => {
                if let Some(location) = panic_info.location() {
                    warn!(
                        "unknown thread panicked at {}:{}:{}: {panic_message}",
                        location.file(),
                        location.line(),
                        location.column()
                    );
                } else {
                    warn!("unknown thread panicked: {panic_message}");
                }
            }
        }

        let trigger_quit = TRIGGER_PANIC_QUIT.swap(false, Ordering::AcqRel);
        if !trigger_quit {
            return;
        }
        do_panic_quit(monitored);
    }));
}

#[cfg(unix)]
fn do_panic_quit(monitored: bool) {
    use rustix::process::Signal;

    if monitored && let Some(pid) = rustix::process::getppid() {
        let _ = rustix::process::kill_process(pid, Signal::HUP);
        return;
    }
    crate::control::quit::trigger_force_shutdown();
}

#[cfg(windows)]
fn do_panic_quit(_monitored: bool) {
    crate::control::quit::trigger_force_shutdown();
}
