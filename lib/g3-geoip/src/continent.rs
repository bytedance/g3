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

pub enum Continent {
    Africa,
    Antarctica,
    Asia,
    Europe,
    NorthAmerica,
    Oceania,
    SouthAmerica,
}

pub struct InvalidContinentCode {}

impl Continent {
    pub fn from_code(s: &str) -> Result<Self, InvalidContinentCode> {
        match s {
            "AF" | "af" => Ok(Continent::Africa),
            "AN" | "an" => Ok(Continent::Antarctica),
            "AS" | "as" => Ok(Continent::Asia),
            "EU" | "eu" => Ok(Continent::Europe),
            "NA" | "na" => Ok(Continent::NorthAmerica),
            "OC" | "oc" => Ok(Continent::Oceania),
            "SA" | "sa" => Ok(Continent::SouthAmerica),
            _ => Err(InvalidContinentCode {}),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Continent::Africa => "Africa",
            Continent::Antarctica => "Antarctica",
            Continent::Asia => "Asia",
            Continent::Europe => "Europe",
            Continent::NorthAmerica => "North America",
            Continent::Oceania => "Oceania",
            Continent::SouthAmerica => "South America",
        }
    }
}
