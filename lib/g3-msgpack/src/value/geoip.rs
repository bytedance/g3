/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::{Context, anyhow};
use rmpv::ValueRef;

use g3_geoip_types::{ContinentCode, IpLocation, IpLocationBuilder, IsoCountryCode};

pub fn as_iso_country_code(value: &ValueRef) -> anyhow::Result<IsoCountryCode> {
    let s = crate::value::as_string(value)
        .context("msgpack 'string' value type is expected for iso country code")?;
    let country = IsoCountryCode::from_str(&s).map_err(|_| anyhow!("invalid iso country code"))?;
    Ok(country)
}

pub fn as_continent_code(value: &ValueRef) -> anyhow::Result<ContinentCode> {
    let s = crate::value::as_string(value)
        .context("msgpack 'string' value type is expected for continent code")?;
    let country = ContinentCode::from_str(&s).map_err(|_| anyhow!("invalid continent code"))?;
    Ok(country)
}

pub fn as_ip_location(value: &ValueRef) -> anyhow::Result<IpLocation> {
    if let ValueRef::Map(map) = value {
        let mut builder = IpLocationBuilder::default();

        for (k, v) in map {
            let k =
                crate::value::as_string(k).context("key of the map is not a valid string value")?;
            match crate::key::normalize(&k).as_str() {
                "network" | "net" => {
                    let net = crate::value::as_ip_network(v)
                        .context(format!("invalid ip network value for key {k}"))?;
                    builder.set_network(net);
                }
                "country" => {
                    let country = as_iso_country_code(v)
                        .context(format!("invalid iso country code value for key {k}"))?;
                    builder.set_country(country);
                }
                "continent" => {
                    let continent = as_continent_code(v)
                        .context(format!("invalid continent code value for key {k}"))?;
                    builder.set_continent(continent);
                }
                "as_number" | "asn" => {
                    let asn = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    builder.set_as_number(asn);
                }
                "isp_name" => {
                    let name = crate::value::as_string(v)
                        .context(format!("invalid string value for key {k}"))?;
                    builder.set_isp_name(name);
                }
                "isp_domain" => {
                    let domain = crate::value::as_string(v)
                        .context(format!("invalid string value for key {k}"))?;
                    builder.set_isp_domain(domain);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        builder.build()
    } else {
        Err(anyhow!(
            "msgpack value type for 'ip location' should be 'map'"
        ))
    }
}
