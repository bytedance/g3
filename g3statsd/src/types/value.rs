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

use std::fmt;
use std::ops;
use std::str::FromStr;

use anyhow::anyhow;
use memchr::memchr;

#[derive(Clone, Copy)]
pub(crate) enum MetricValue {
    Double(f64),
    Signed(i64),
    Unsigned(u64),
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
