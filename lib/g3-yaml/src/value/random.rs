/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use rand::distr::Bernoulli;
use yaml_rust::Yaml;

pub fn as_random_ratio(value: &Yaml) -> anyhow::Result<Bernoulli> {
    match value {
        Yaml::Real(s) => {
            let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 ratio value: {e}"))?;
            Bernoulli::new(f).map_err(|e| anyhow!("out of range f64 ratio: {e}"))
        }
        Yaml::Integer(i) => match i {
            0 => Ok(Bernoulli::new(0.0).unwrap()),
            1 => Ok(Bernoulli::new(1.0).unwrap()),
            _ => Err(anyhow!("out of range integer value, only 0 & 1 is allowed")),
        },
        Yaml::String(s) => {
            if let Some(p) = s.find('/') {
                let n1 = u32::from_str(s[0..p].trim())
                    .map_err(|e| anyhow!("first part is not valid u32: {e}"))?;
                let n2 = u32::from_str(s[p + 1..].trim())
                    .map_err(|e| anyhow!("second part is not valid u32: {e}"))?;
                Bernoulli::from_ratio(n1, n2)
                    .map_err(|e| anyhow!("out of range fraction ratio: {e}"))
            } else if let Some(s) = s.strip_suffix('%') {
                let n = u32::from_str(s.trim())
                    .map_err(|e| anyhow!("the part before % is not valid u32: {e}"))?;
                Bernoulli::from_ratio(n, 100)
                    .map_err(|e| anyhow!("out of range percentage ratio: {e}"))
            } else {
                let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 ratio string: {e}"))?;
                Bernoulli::new(f).map_err(|e| anyhow!("out of range f64 ratio: {e}"))
            }
        }
        Yaml::Boolean(true) => Ok(Bernoulli::new(1.0).unwrap()),
        Yaml::Boolean(false) => Ok(Bernoulli::new(0.0).unwrap()),
        _ => Err(anyhow!(
            "yaml value type for 'random ratio' should be 'f64' or 'string'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_random_ratio_ok() {
        // valid Real values
        let yaml = Yaml::Real("0.5".into());
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 0.5);

        let yaml = Yaml::Real("1.0".into());
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 1.0);

        // valid Integer values (0 and 1 only)
        let yaml = Yaml::Integer(0);
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 0.0);

        let yaml = Yaml::Integer(1);
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 1.0);

        // valid String formats
        // Fraction format
        let yaml = yaml_str!("1/2");
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 0.5);

        // Percentage format
        let yaml = yaml_str!("50%");
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 0.5);

        // Float string format
        let yaml = yaml_str!("0.7");
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 0.7);

        // valid Boolean values
        let yaml = Yaml::Boolean(true);
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 1.0);

        let yaml = Yaml::Boolean(false);
        assert_eq!(as_random_ratio(&yaml).unwrap().p(), 0.0);
    }

    #[test]
    fn as_random_ratio_err() {
        // invalid Real values
        let yaml = Yaml::Real("abc".into());
        assert!(as_random_ratio(&yaml).is_err());

        let yaml = Yaml::Real("-1.0".into());
        assert!(as_random_ratio(&yaml).is_err());

        let yaml = Yaml::Real("2.5".into());
        assert!(as_random_ratio(&yaml).is_err());

        // invalid Integer values (not 0 or 1)
        let yaml = Yaml::Integer(2);
        assert!(as_random_ratio(&yaml).is_err());

        // invalid String formats
        // Invalid fraction format
        let yaml = yaml_str!("a/2");
        assert!(as_random_ratio(&yaml).is_err());

        let yaml = yaml_str!("1/b");
        assert!(as_random_ratio(&yaml).is_err());

        let yaml = yaml_str!("3/0");
        assert!(as_random_ratio(&yaml).is_err());

        // Invalid percentage format
        let yaml = yaml_str!("a%");
        assert!(as_random_ratio(&yaml).is_err());

        let yaml = yaml_str!("150%");
        assert!(as_random_ratio(&yaml).is_err());

        // Invalid float string
        let yaml = yaml_str!("abc");
        assert!(as_random_ratio(&yaml).is_err());

        let yaml = yaml_str!("-0.5");
        assert!(as_random_ratio(&yaml).is_err());

        // unsupported types
        let yaml = Yaml::Array(vec![]);
        assert!(as_random_ratio(&yaml).is_err());

        let yaml = Yaml::Null;
        assert!(as_random_ratio(&yaml).is_err());
    }
}
