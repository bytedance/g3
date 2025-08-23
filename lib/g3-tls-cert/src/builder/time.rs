/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
