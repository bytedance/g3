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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PortRange {
    start: u16,
    end: u16,
}

impl PortRange {
    pub fn new(port_start: u16, port_end: u16) -> Self {
        PortRange {
            start: port_start,
            end: port_end,
        }
    }

    #[inline]
    pub fn count(&self) -> u16 {
        self.end - self.start + 1
    }

    #[inline]
    pub fn start(&self) -> u16 {
        self.start
    }

    #[inline]
    pub fn end(&self) -> u16 {
        self.end
    }

    pub fn check(&self) -> anyhow::Result<()> {
        if self.start == 0 {
            return Err(anyhow!("the start port should not be 0"));
        }

        if self.end <= self.start {
            return Err(anyhow!(
                "the end port should be greater than the start port"
            ));
        }

        Ok(())
    }
}

impl FromStr for PortRange {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((prefix, suffix)) = s.split_once('-') {
            let start = u16::from_str(prefix.trim())
                .map_err(|e| anyhow!("the prefix part is not a valid port number: {e}"))?;
            let end = u16::from_str(suffix.trim())
                .map_err(|e| anyhow!("the suffix part is not a valid port number: {e}"))?;

            let range = PortRange { start, end };
            range.check()?;
            Ok(range)
        } else {
            Err(anyhow!("no delimiter found"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal() {
        let r = PortRange::new(61000, 62000);

        let range = PortRange::from_str("61000-62000").unwrap();
        assert_eq!(r, range);

        let range = PortRange::from_str("61000 - 62000").unwrap();
        assert_eq!(r, range);
    }

    #[test]
    fn parse_error() {
        assert!(PortRange::from_str("61000").is_err());
        assert!(PortRange::from_str("0-10000").is_err());
        assert!(PortRange::from_str("1-1").is_err());
    }
}
