/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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

    pub(crate) fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
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
