/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

    if monitored {
        if let Some(pid) = rustix::process::getppid() {
            let _ = rustix::process::kill_process(pid, Signal::HUP);
            return;
        }
    }
    crate::control::quit::trigger_force_shutdown();
}

#[cfg(windows)]
fn do_panic_quit(_monitored: bool) {
    crate::control::quit::trigger_force_shutdown();
}
