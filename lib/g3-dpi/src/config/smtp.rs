/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmtpInterceptionConfig {
    pub greeting_timeout: Duration,
    pub quit_wait_timeout: Duration,
    pub command_wait_timeout: Duration,
    pub response_wait_timeout: Duration,
    pub data_initiation_timeout: Duration,
    pub data_termination_timeout: Duration,
    pub allow_on_demand_mail_relay: bool,
    pub allow_data_chunking: bool,
    pub allow_burl_data: bool,
}

impl Default for SmtpInterceptionConfig {
    fn default() -> Self {
        SmtpInterceptionConfig {
            greeting_timeout: Duration::from_secs(300),
            quit_wait_timeout: Duration::from_secs(60),
            command_wait_timeout: Duration::from_secs(300),
            response_wait_timeout: Duration::from_secs(300),
            data_initiation_timeout: Duration::from_secs(120),
            data_termination_timeout: Duration::from_secs(600),
            allow_on_demand_mail_relay: false,
            allow_data_chunking: false,
            allow_burl_data: false,
        }
    }
}
