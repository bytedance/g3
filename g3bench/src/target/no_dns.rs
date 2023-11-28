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

use std::sync::Arc;

use anyhow::anyhow;
use clap::{ArgMatches, Command};

use crate::ProcArgs;

pub const COMMAND: &str = "dns";

pub fn command() -> Command {
    Command::new(COMMAND).hide(true)
}

pub async fn run(_proc_args: &Arc<ProcArgs>, _cmd_args: &ArgMatches) -> anyhow::Result<()> {
    Err(anyhow!(
        "dns support is not compiled in, 'hickory' feature is needed to enable this"
    ))
}
