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

use std::path::PathBuf;

#[derive(Debug)]
pub struct DaemonArgs {
    pub with_systemd: bool,
    pub daemon_mode: bool,
    pub verbose_level: u8,
    pub process_name: &'static str,
    pub pid_file: Option<PathBuf>,
}

impl DaemonArgs {
    pub fn new(process_name: &'static str) -> Self {
        DaemonArgs {
            with_systemd: false,
            daemon_mode: false,
            verbose_level: 0,
            process_name,
            pid_file: None,
        }
    }
}
