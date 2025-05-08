/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
use yaml_rust::Yaml;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TimestampPrecision {
    Seconds,
    MilliSeconds,
    MicroSeconds,
    NanoSeconds,
}

impl TimestampPrecision {
    pub(crate) fn v2_query_value(self) -> &'static str {
        match self {
            Self::Seconds => "s",
            Self::MilliSeconds => "ms",
            Self::MicroSeconds => "us",
            Self::NanoSeconds => "ns",
        }
    }

    pub(crate) fn v3_query_value(self) -> &'static str {
        match self {
            Self::Seconds => "second",
            Self::MilliSeconds => "millisecond",
            Self::MicroSeconds => "microsecond",
            Self::NanoSeconds => "nanosecond",
        }
    }

    pub(super) fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::String(s) = value {
            TimestampPrecision::from_str(s)
        } else {
            Err(anyhow!(
                "yaml value type for timestamp precision should be string"
            ))
        }
    }
}

impl FromStr for TimestampPrecision {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "s" | "second" | "seconds" => Ok(TimestampPrecision::Seconds),
            "ms" | "millisecond" | "milliseconds" => Ok(TimestampPrecision::MilliSeconds),
            "us" | "microsecond" | "microseconds" => Ok(TimestampPrecision::MicroSeconds),
            "ns" | "nanosecond" | "nanoseconds" => Ok(TimestampPrecision::NanoSeconds),
            _ => Err(anyhow!("invalid timestamp precision: {s}")),
        }
    }
}
