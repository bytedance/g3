/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::str::FromStr;

const ALL_CONTINENT_NAMES: &[&str] = &[
    "Africa",
    "Antarctica",
    "Asia",
    "Europe",
    "North America",
    "Oceania",
    "South America",
];

const ALL_CONTINENT_CODES: &[&str] = &["AF", "AN", "AS", "EU", "NA", "OC", "SA"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ContinentCode {
    AF,
    AN,
    AS,
    EU,
    NA,
    OC,
    SA,
}

impl ContinentCode {
    pub fn name(&self) -> &'static str {
        ALL_CONTINENT_NAMES[*self as usize]
    }

    pub fn code(&self) -> &'static str {
        ALL_CONTINENT_CODES[*self as usize]
    }

    pub fn variant_count() -> usize {
        Self::SA as usize
    }
}

impl FromStr for ContinentCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AF" | "af" => Ok(ContinentCode::AF),
            "AN" | "an" => Ok(ContinentCode::AN),
            "AS" | "as" => Ok(ContinentCode::AS),
            "EU" | "eu" => Ok(ContinentCode::EU),
            "NA" | "na" => Ok(ContinentCode::NA),
            "OC" | "oc" => Ok(ContinentCode::OC),
            "SA" | "sa" => Ok(ContinentCode::SA),
            _ => Err(()),
        }
    }
}

impl fmt::Display for ContinentCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Continent {
    Africa,
    Antarctica,
    Asia,
    Europe,
    NorthAmerica,
    Oceania,
    SouthAmerica,
}

impl From<ContinentCode> for Continent {
    fn from(value: ContinentCode) -> Self {
        match value {
            ContinentCode::AF => Continent::Africa,
            ContinentCode::AN => Continent::Antarctica,
            ContinentCode::AS => Continent::Asia,
            ContinentCode::EU => Continent::Europe,
            ContinentCode::NA => Continent::NorthAmerica,
            ContinentCode::OC => Continent::Oceania,
            ContinentCode::SA => Continent::SouthAmerica,
        }
    }
}

impl Continent {
    pub fn name(&self) -> &'static str {
        ALL_CONTINENT_NAMES[*self as usize]
    }
}

impl From<Continent> for ContinentCode {
    fn from(value: Continent) -> Self {
        match value {
            Continent::Africa => ContinentCode::AF,
            Continent::Antarctica => ContinentCode::AN,
            Continent::Asia => ContinentCode::AS,
            Continent::Europe => ContinentCode::EU,
            Continent::NorthAmerica => ContinentCode::NA,
            Continent::Oceania => ContinentCode::OC,
            Continent::SouthAmerica => ContinentCode::SA,
        }
    }
}
