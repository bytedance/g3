/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{register_offline, register_quit, register_reload};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::register_quit;

pub trait AsyncSignalAction: Copy {
    fn run(&self) -> impl Future<Output = ()> + Send;
}
