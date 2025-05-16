/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashSet;
use std::ops::RangeInclusive;
use std::str::FromStr;

use anyhow::anyhow;
use fnv::{FnvBuildHasher, FnvHashSet};

#[derive(Clone, Eq, PartialEq)]
pub struct Ports(FnvHashSet<u16>);

impl Default for Ports {
    fn default() -> Self {
        Ports(HashSet::with_hasher(FnvBuildHasher::default()))
    }
}

impl Ports {
    pub fn add_single(&mut self, port: u16) {
        self.0.insert(port);
    }

    pub fn add_range(&mut self, start: u16, end: u16) {
        let range = RangeInclusive::new(start, end);
        for port in range {
            self.add_single(port);
        }
    }

    pub fn contains(&self, port: u16) -> bool {
        self.0.contains(&port)
    }
}

impl FromStr for Ports {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let sc: Vec<&str> = s.split(',').collect();

        let mut ports = Ports::default();
        for (i, part) in sc.iter().enumerate() {
            let sd: Vec<&str> = part.trim().split('-').collect();

            match sd.len() {
                1 => {
                    let port = u16::from_str(sd[0].trim())
                        .map_err(|e| anyhow!("#{i} is invalid port: {e}"))?;
                    ports.add_single(port);
                }
                2 => {
                    let port_start = u16::from_str(sd[0].trim())
                        .map_err(|e| anyhow!("#{i} is invalid start port: {e}"))?;
                    let port_end = u16::from_str(sd[1].trim())
                        .map_err(|e| anyhow!("#{i} is invalid end port: {e}"))?;
                    if port_start > port_end {
                        return Err(anyhow!("start port is greater than end port"));
                    }
                    ports.add_range(port_start, port_end);
                }
                _ => return Err(anyhow!("#{i} contains too many '-'")),
            }
        }

        Ok(ports)
    }
}

impl IntoIterator for Ports {
    type Item = u16;
    type IntoIter = std::collections::hash_set::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Extend<u16> for Ports {
    fn extend<T: IntoIterator<Item = u16>>(&mut self, iter: T) {
        for port in iter {
            self.add_single(port);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_str() {
        let ports = Ports::from_str("1234").unwrap();
        assert!(ports.contains(1234));
    }

    #[test]
    fn test_range_str() {
        let ports = Ports::from_str("10000-10100").unwrap();
        assert!(ports.contains(10000));
        assert!(ports.contains(10010));
        assert!(ports.contains(10100));
    }

    #[test]
    fn test_range_err() {
        let r = Ports::from_str("10000-9000");
        assert!(r.is_err());
    }

    #[test]
    fn test_discrete_str() {
        let ports = Ports::from_str("80,443").unwrap();
        assert!(ports.contains(80));
        assert!(ports.contains(443));
    }

    #[test]
    fn test_mixed_str() {
        let ports = Ports::from_str("8080, 9000 - 9100").unwrap();
        assert!(ports.contains(8080));
        assert!(ports.contains(9000));
        assert!(ports.contains(9050));
        assert!(ports.contains(9100));
    }
}
