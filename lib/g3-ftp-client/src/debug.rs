/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use log::Level;

pub const FTP_DEBUG_LOG_LEVEL: Level = Level::Debug;
pub const FTP_DEBUG_LOG_TARGET: &str = "";

#[macro_export]
macro_rules! log_msg {
    ($s:literal, $($arg:tt)+) => (
        log::log!(target: $crate::FTP_DEBUG_LOG_TARGET, $crate::FTP_DEBUG_LOG_LEVEL, concat!(": ", $s), $($arg)+)
    )
}

#[cfg(feature = "log-raw-io")]
#[inline]
pub(crate) fn log_cmd(cmd: &str) {
    log::log!(
        target: FTP_DEBUG_LOG_TARGET,
        FTP_DEBUG_LOG_LEVEL,
        "> {cmd}",
    );
}

#[cfg(feature = "log-raw-io")]
#[inline]
pub(crate) fn log_rsp(rsp: &str) {
    log::log!(
        target: FTP_DEBUG_LOG_TARGET,
        FTP_DEBUG_LOG_LEVEL,
        "< {rsp}",
    );
}
