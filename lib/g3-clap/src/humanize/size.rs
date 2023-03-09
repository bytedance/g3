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

use anyhow::anyhow;
use clap::ArgMatches;
use humanize_rs::bytes::Bytes;

pub fn get_usize(args: &ArgMatches, id: &str) -> anyhow::Result<Option<usize>> {
    if let Some(v) = args.get_one::<String>(id) {
        if let Ok(b) = v.parse::<Bytes>() {
            Ok(Some(b.size()))
        } else if let Ok(size) = usize::from_str(v) {
            Ok(Some(size))
        } else {
            Err(anyhow!("invalid {id} value {v}"))
        }
    } else {
        Ok(None)
    }
}
