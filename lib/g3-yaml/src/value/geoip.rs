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
use yaml_rust::Yaml;

use g3_geoip::{ContinentCode, IsoCountryCode};

pub fn as_iso_country_code(value: &Yaml) -> anyhow::Result<IsoCountryCode> {
    if let Yaml::String(s) = value {
        let country =
            IsoCountryCode::from_str(s).map_err(|_| anyhow!("invalid iso country code"))?;
        Ok(country)
    } else {
        Err(anyhow!(
            "yaml value type for 'iso country code' should be 'string'"
        ))
    }
}

pub fn as_continent_code(value: &Yaml) -> anyhow::Result<ContinentCode> {
    if let Yaml::String(s) = value {
        let country = ContinentCode::from_str(s).map_err(|_| anyhow!("invalid continent code"))?;
        Ok(country)
    } else {
        Err(anyhow!(
            "yaml value type for 'continent code' should be 'string'"
        ))
    }
}
