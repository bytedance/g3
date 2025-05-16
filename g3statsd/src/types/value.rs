/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::fmt::{self, Write};
use std::ops;
use std::str::FromStr;

use anyhow::anyhow;
use memchr::memchr;
use serde_json::Number;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MetricValue {
    Double(f64),
    Signed(i64),
    Unsigned(u64),
}

impl MetricValue {
    pub(crate) fn display_influxdb(&self) -> DisplayInfluxdbValue {
        DisplayInfluxdbValue(self)
    }

    #[allow(unused)]
    pub(crate) fn as_f64(&self) -> f64 {
        match self {
            MetricValue::Double(f) => *f,
            MetricValue::Signed(i) => *i as f64,
            MetricValue::Unsigned(u) => *u as f64,
        }
    }

    pub(crate) fn as_json_number(&self) -> Number {
        match self {
            MetricValue::Double(f) => Number::from_f64(*f).unwrap(),
            MetricValue::Signed(i) => Number::from(*i),
            MetricValue::Unsigned(u) => Number::from(*u),
        }
    }
}

impl FromStr for MetricValue {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(anyhow!("empty string"));
        }

        if s.as_bytes()[0] == b'-' {
            match memchr(b'.', s.as_bytes()) {
                Some(_) => {
                    let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 string: {e}"))?;
                    Ok(MetricValue::Double(f))
                }
                None => {
                    let i = i64::from_str(s).map_err(|e| anyhow!("invalid i64 string: {e}"))?;
                    Ok(MetricValue::Signed(i))
                }
            }
        } else {
            match memchr(b'.', s.as_bytes()) {
                Some(_) => {
                    let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 string: {e}"))?;
                    Ok(MetricValue::Double(f))
                }
                None => {
                    let u = u64::from_str(s).map_err(|e| anyhow!("invalid u64 string: {e}"))?;
                    Ok(MetricValue::Unsigned(u))
                }
            }
        }
    }
}

impl fmt::Display for MetricValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricValue::Unsigned(u) => itoa::Buffer::new().format(*u).fmt(f),
            MetricValue::Signed(i) => itoa::Buffer::new().format(*i).fmt(f),
            MetricValue::Double(v) => ryu::Buffer::new().format(*v).fmt(f),
        }
    }
}

impl ops::Add for MetricValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (MetricValue::Unsigned(u1), MetricValue::Unsigned(u2)) => {
                MetricValue::Unsigned(u1.wrapping_add(u2))
            }
            (MetricValue::Unsigned(u1), MetricValue::Signed(i2)) => {
                MetricValue::Signed(i2.wrapping_add_unsigned(u1))
            }
            (MetricValue::Unsigned(u1), MetricValue::Double(f2)) => {
                MetricValue::Double(f2 + u1 as f64)
            }
            (MetricValue::Signed(i1), MetricValue::Unsigned(u2)) => {
                MetricValue::Signed(i1.wrapping_add_unsigned(u2))
            }
            (MetricValue::Signed(i1), MetricValue::Signed(i2)) => {
                MetricValue::Signed(i1.wrapping_add(i2))
            }
            (MetricValue::Signed(i1), MetricValue::Double(f2)) => {
                MetricValue::Double(f2 + i1 as f64)
            }
            (MetricValue::Double(f1), MetricValue::Unsigned(u2)) => {
                MetricValue::Double(f1 + u2 as f64)
            }
            (MetricValue::Double(f1), MetricValue::Signed(i2)) => {
                MetricValue::Double(f1 + i2 as f64)
            }
            (MetricValue::Double(f1), MetricValue::Double(f2)) => MetricValue::Double(f1 + f2),
        }
    }
}

impl ops::AddAssign for MetricValue {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

pub(crate) struct DisplayInfluxdbValue<'a>(&'a MetricValue);

impl fmt::Display for DisplayInfluxdbValue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            MetricValue::Unsigned(u) => {
                itoa::Buffer::new().format(*u).fmt(f)?;
                f.write_char('u')
            }
            MetricValue::Signed(i) => {
                itoa::Buffer::new().format(*i).fmt(f)?;
                f.write_char('i')
            }
            MetricValue::Double(v) => ryu::Buffer::new().format(*v).fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn influxdb() {
        let v = MetricValue::Unsigned(10);
        assert_eq!(v.display_influxdb().to_string(), "10u");

        let v = MetricValue::Signed(10);
        assert_eq!(v.display_influxdb().to_string(), "10i");

        let v = MetricValue::Double(1.0);
        assert_eq!(v.display_influxdb().to_string(), "1.0");
    }
}
