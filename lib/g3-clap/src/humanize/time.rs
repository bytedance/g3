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

use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use clap::ArgMatches;

pub fn get_duration(args: &ArgMatches, id: &str) -> anyhow::Result<Option<Duration>> {
    if let Some(v) = args.get_one::<String>(id) {
        if let Ok(timeout) = humanize_rs::duration::parse(v) {
            Ok(Some(timeout))
        } else if let Ok(timeout) = u64::from_str(v) {
            Ok(Some(Duration::from_secs(timeout)))
        } else if let Ok(timeout) = f64::from_str(v) {
            let timeout = Duration::try_from_secs_f64(timeout)
                .map_err(|e| anyhow!("out of range timeout value: {e}"))?;
            Ok(Some(timeout))
        } else {
            Err(anyhow!("invalid {id} value {v}"))
        }
    } else {
        Ok(None)
    }
}
