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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continent_code_name() {
        assert_eq!(ContinentCode::AF.name(), "Africa");
        assert_eq!(ContinentCode::AN.name(), "Antarctica");
        assert_eq!(ContinentCode::AS.name(), "Asia");
        assert_eq!(ContinentCode::EU.name(), "Europe");
        assert_eq!(ContinentCode::NA.name(), "North America");
        assert_eq!(ContinentCode::OC.name(), "Oceania");
        assert_eq!(ContinentCode::SA.name(), "South America");
    }

    #[test]
    fn continent_code_code() {
        assert_eq!(ContinentCode::AF.code(), "AF");
        assert_eq!(ContinentCode::AN.code(), "AN");
        assert_eq!(ContinentCode::AS.code(), "AS");
        assert_eq!(ContinentCode::EU.code(), "EU");
        assert_eq!(ContinentCode::NA.code(), "NA");
        assert_eq!(ContinentCode::OC.code(), "OC");
        assert_eq!(ContinentCode::SA.code(), "SA");
    }

    #[test]
    fn continent_code_variant_count() {
        assert_eq!(ContinentCode::variant_count(), 6);
    }

    #[test]
    fn continent_code_from_str() {
        assert_eq!(ContinentCode::from_str("AF").unwrap(), ContinentCode::AF);
        assert_eq!(ContinentCode::from_str("AN").unwrap(), ContinentCode::AN);
        assert_eq!(ContinentCode::from_str("AS").unwrap(), ContinentCode::AS);
        assert_eq!(ContinentCode::from_str("EU").unwrap(), ContinentCode::EU);
        assert_eq!(ContinentCode::from_str("NA").unwrap(), ContinentCode::NA);
        assert_eq!(ContinentCode::from_str("OC").unwrap(), ContinentCode::OC);
        assert_eq!(ContinentCode::from_str("SA").unwrap(), ContinentCode::SA);

        assert_eq!(ContinentCode::from_str("af").unwrap(), ContinentCode::AF);
        assert_eq!(ContinentCode::from_str("an").unwrap(), ContinentCode::AN);
        assert_eq!(ContinentCode::from_str("as").unwrap(), ContinentCode::AS);
        assert_eq!(ContinentCode::from_str("eu").unwrap(), ContinentCode::EU);
        assert_eq!(ContinentCode::from_str("na").unwrap(), ContinentCode::NA);
        assert_eq!(ContinentCode::from_str("oc").unwrap(), ContinentCode::OC);
        assert_eq!(ContinentCode::from_str("sa").unwrap(), ContinentCode::SA);

        assert!(ContinentCode::from_str("XX").is_err());
        assert!(ContinentCode::from_str("").is_err());
        assert!(ContinentCode::from_str("Africa").is_err());
        assert!(ContinentCode::from_str("asia").is_err());
        assert!(ContinentCode::from_str("EUROPE").is_err());
        assert!(ContinentCode::from_str("123").is_err());
        assert!(ContinentCode::from_str("AF ").is_err());
        assert!(ContinentCode::from_str(" AF").is_err());
    }

    #[test]
    fn continent_from_continent_code() {
        assert!(matches!(
            Continent::from(ContinentCode::AF),
            Continent::Africa
        ));
        assert!(matches!(
            Continent::from(ContinentCode::AN),
            Continent::Antarctica
        ));
        assert!(matches!(
            Continent::from(ContinentCode::AS),
            Continent::Asia
        ));
        assert!(matches!(
            Continent::from(ContinentCode::EU),
            Continent::Europe
        ));
        assert!(matches!(
            Continent::from(ContinentCode::NA),
            Continent::NorthAmerica
        ));
        assert!(matches!(
            Continent::from(ContinentCode::OC),
            Continent::Oceania
        ));
        assert!(matches!(
            Continent::from(ContinentCode::SA),
            Continent::SouthAmerica
        ));
    }

    #[test]
    fn continent_name() {
        assert_eq!(Continent::Africa.name(), "Africa");
        assert_eq!(Continent::Antarctica.name(), "Antarctica");
        assert_eq!(Continent::Asia.name(), "Asia");
        assert_eq!(Continent::Europe.name(), "Europe");
        assert_eq!(Continent::NorthAmerica.name(), "North America");
        assert_eq!(Continent::Oceania.name(), "Oceania");
        assert_eq!(Continent::SouthAmerica.name(), "South America");
    }

    #[test]
    fn continent_code_from_continent() {
        assert_eq!(ContinentCode::from(Continent::Africa), ContinentCode::AF);
        assert_eq!(
            ContinentCode::from(Continent::Antarctica),
            ContinentCode::AN
        );
        assert_eq!(ContinentCode::from(Continent::Asia), ContinentCode::AS);
        assert_eq!(ContinentCode::from(Continent::Europe), ContinentCode::EU);
        assert_eq!(
            ContinentCode::from(Continent::NorthAmerica),
            ContinentCode::NA
        );
        assert_eq!(ContinentCode::from(Continent::Oceania), ContinentCode::OC);
        assert_eq!(
            ContinentCode::from(Continent::SouthAmerica),
            ContinentCode::SA
        );
    }

    #[test]
    fn continent_code_ordering() {
        assert!(ContinentCode::AF < ContinentCode::AN);
        assert!(ContinentCode::AN < ContinentCode::AS);
        assert!(ContinentCode::AS < ContinentCode::EU);
        assert!(ContinentCode::EU < ContinentCode::NA);
        assert!(ContinentCode::NA < ContinentCode::OC);
        assert!(ContinentCode::OC < ContinentCode::SA);
    }
}
