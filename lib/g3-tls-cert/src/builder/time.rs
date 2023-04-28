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

use anyhow::anyhow;
use chrono::{DateTime, Datelike, Utc};
use openssl::asn1::Asn1Time;

pub(super) fn asn1_time_from_chrono(datetime: &DateTime<Utc>) -> anyhow::Result<Asn1Time> {
    let lazy_fmt = if datetime.year() >= 2050 {
        datetime.format_with_items(g3_datetime::format::asn1::RFC5280_GENERALIZED.iter())
    } else {
        datetime.format_with_items(g3_datetime::format::asn1::RFC5280_UTC.iter())
    };
    Asn1Time::from_str(&format!("{lazy_fmt}")).map_err(|e| anyhow!("failed to get asn1 time: {e}"))
}
