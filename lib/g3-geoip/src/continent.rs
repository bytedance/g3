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

const ALL_CONTINENT_NAMES: &[&str] = &[
    "Africa",
    "Antarctica",
    "Asia",
    "Europe",
    "North America",
    "Oceania",
    "South America",
];

#[derive(Debug, Clone, Copy)]
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
