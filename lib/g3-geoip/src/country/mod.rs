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

use crate::ContinentCode;

mod generated;
pub use generated::{ISO3166Alpha2CountryCode, ISO3166Alpha3CountryCode};

#[derive(Debug, Clone, Copy)]
pub enum CountryCode {
    ISO3166Alpha2(ISO3166Alpha2CountryCode),
    ISO3166Alpha3(ISO3166Alpha3CountryCode),
}

impl CountryCode {
    pub fn name(&self) -> &'static str {
        match self {
            CountryCode::ISO3166Alpha2(c) => c.name(),
            CountryCode::ISO3166Alpha3(c) => c.name(),
        }
    }

    pub fn continent(&self) -> ContinentCode {
        match self {
            CountryCode::ISO3166Alpha2(c) => c.continent(),
            CountryCode::ISO3166Alpha3(c) => c.continent(),
        }
    }
}

impl FromStr for CountryCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.len() {
            2 => {
                let code = ISO3166Alpha2CountryCode::from_str(s)?;
                Ok(CountryCode::ISO3166Alpha2(code))
            }
            3 => {
                let code = ISO3166Alpha3CountryCode::from_str(s)?;
                Ok(CountryCode::ISO3166Alpha3(code))
            }
            _ => Err(()),
        }
    }
}
