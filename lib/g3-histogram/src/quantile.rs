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

use std::borrow::Cow;
use std::cmp::Ordering;
use std::num::ParseFloatError;
use std::str::FromStr;
use std::string::ToString;

use ryu::Buffer;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InvalidQuantile {
    #[error("invalid float value: {0}")]
    InvalidFloat(#[from] ParseFloatError),
    #[error("out of range(0.0-1.0) value {0}")]
    OutOfRange(f64),
}

#[derive(Clone, Debug)]
pub struct Quantile {
    v: f64,
    s: Cow<'static, str>,
}

impl Quantile {
    pub const PCT50: Quantile = Quantile {
        v: 0.50,
        s: Cow::Borrowed("0.50"),
    };

    pub const PCT80: Quantile = Quantile {
        v: 0.80,
        s: Cow::Borrowed("0.80"),
    };

    pub const PCT90: Quantile = Quantile {
        v: 0.90,
        s: Cow::Borrowed("0.90"),
    };

    pub const PCT95: Quantile = Quantile {
        v: 0.95,
        s: Cow::Borrowed("0.95"),
    };

    pub const PCT99: Quantile = Quantile {
        v: 0.99,
        s: Cow::Borrowed("0.99"),
    };

    #[inline]
    pub fn as_str(&self) -> &str {
        self.s.as_ref()
    }

    #[inline]
    pub fn value(&self) -> f64 {
        self.v
    }
}

fn quantile_in_range(v: f64) -> Result<(), InvalidQuantile> {
    if (0.0_f64..=1.0_f64).contains(&v) {
        Ok(())
    } else {
        Err(InvalidQuantile::OutOfRange(v))
    }
}

impl TryFrom<f64> for Quantile {
    type Error = InvalidQuantile;

    fn try_from(v: f64) -> Result<Self, Self::Error> {
        quantile_in_range(v)?;
        let mut b = Buffer::new();
        let s = b.format_finite(v).to_string();
        Ok(Quantile {
            v,
            s: Cow::Owned(s),
        })
    }
}

impl FromStr for Quantile {
    type Err = InvalidQuantile;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = f64::from_str(s)?;
        quantile_in_range(v)?;
        Ok(Quantile {
            v,
            s: Cow::Owned(s.to_string()),
        })
    }
}

impl PartialEq for Quantile {
    fn eq(&self, other: &Self) -> bool {
        self.v.eq(&other.v)
    }
}

impl Eq for Quantile {}

impl PartialOrd for Quantile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Quantile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.v.total_cmp(&other.v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_f64() {
        let quantile = Quantile::try_from(1.0).unwrap();
        assert_eq!(quantile.as_str(), "1.0");

        let quantile = Quantile::try_from(0.5).unwrap();
        assert_eq!(quantile.as_str(), "0.5");

        let quantile = Quantile::try_from(0.999).unwrap();
        assert_eq!(quantile.as_str(), "0.999");
    }

    #[test]
    fn fmt_str() {
        let quantile = Quantile::from_str("1.0").unwrap();
        assert_eq!(quantile.value(), 1.0);
        assert_eq!(quantile.as_str(), "1.0");

        let quantile = Quantile::from_str("0.50").unwrap();
        assert_eq!(quantile.value(), 0.5);
        assert_eq!(quantile.as_str(), "0.50");

        let quantile = Quantile::from_str("0.999").unwrap();
        assert_eq!(quantile.value(), 0.999);
        assert_eq!(quantile.as_str(), "0.999");
    }
}
