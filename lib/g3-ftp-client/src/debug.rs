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
        "> {}",
        cmd
    );
}

#[cfg(feature = "log-raw-io")]
#[inline]
pub(crate) fn log_rsp(rsp: &str) {
    log::log!(
        target: FTP_DEBUG_LOG_TARGET,
        FTP_DEBUG_LOG_LEVEL,
        "< {}",
        rsp
    );
}
