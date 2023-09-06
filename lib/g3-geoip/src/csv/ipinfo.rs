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

use std::net::IpAddr;
use std::ops::BitXor;
use std::path::Path;
use std::str::FromStr;

use anyhow::anyhow;
use ip_network::{IpNetwork, Ipv4Network, Ipv6Network};
use ip_network_table::IpNetworkTable;

use crate::{ContinentCode, CountryCode, GeoIpRecord};

pub fn load(file: &Path) -> anyhow::Result<IpNetworkTable<GeoIpRecord>> {
    let mut table = IpNetworkTable::new();

    let mut rdr = csv::Reader::from_path(file)
        .map_err(|e| anyhow!("failed to read csv file {}: {e}", file.display()))?;
    let headers = rdr
        .headers()
        .map_err(|e| anyhow!("no csv header line found: {e}"))?;

    let mut start_ip_index = usize::MAX;
    let mut end_ip_index = usize::MAX;
    let mut country_index = usize::MAX;
    let mut continent_index = usize::MAX;
    let mut asn_index = usize::MAX;
    let mut as_name_index = usize::MAX;
    let mut as_domain_index = usize::MAX;
    for (column, s) in headers.iter().enumerate() {
        match s {
            "start_ip" => start_ip_index = column,
            "end_ip" => end_ip_index = column,
            "country" => country_index = column,
            "continent" => continent_index = column,
            "asn" => asn_index = column,
            "as_name" => as_name_index = column,
            "as_domain" => as_domain_index = column,
            _ => {}
        }
    }

    for (i, record) in rdr.records().enumerate() {
        let record = record.map_err(|e| anyhow!("invalid record {i}: {e}"))?;

        macro_rules! get_field {
            ($field:ident, $index:expr) => {
                let Some($field) = record.get($index) else {
                    continue;
                };
            };
        }

        get_field!(start_ip, start_ip_index);
        let Ok(start_ip) = IpAddr::from_str(start_ip) else {
            continue;
        };
        get_field!(end_ip, end_ip_index);
        let Ok(end_ip) = IpAddr::from_str(end_ip) else {
            continue;
        };

        let network = match (start_ip, end_ip) {
            (IpAddr::V4(s), IpAddr::V4(e)) => {
                let si = u32::from(s);
                let ei = u32::from(e);
                let prefix = si.bitxor(&ei).leading_zeros() as u8;
                // the start ip may be not the network address with trailing zeros, so truncate here
                let Ok(v4_net) = Ipv4Network::new_truncate(s, prefix) else {
                    continue;
                };
                IpNetwork::V4(v4_net)
            }
            (IpAddr::V6(s), IpAddr::V6(e)) => {
                let si = u128::from(s);
                let ei = u128::from(e);
                let prefix = si.bitxor(&ei).leading_zeros() as u8;
                // the start ip may be not the network address with trailing zeros, so truncate here
                let Ok(v6_net) = Ipv6Network::new_truncate(s, prefix) else {
                    continue;
                };
                IpNetwork::V6(v6_net)
            }
            _ => continue,
        };

        get_field!(country, country_index);
        let Ok(country) = CountryCode::from_str(country) else {
            continue;
        };
        get_field!(continent, continent_index);
        let Ok(continent) = ContinentCode::from_str(continent) else {
            continue;
        };
        let asn = record
            .get(asn_index)
            .and_then(|v| u32::from_str(v.strip_prefix("AS").unwrap_or(v)).ok());
        let as_name = record.get(as_name_index).map(|s| s.to_string());
        let as_domain = record.get(as_domain_index).map(|s| s.to_string());

        let geo_record = GeoIpRecord {
            network,
            country,
            continent,
            as_number: asn,
            as_name,
            as_domain,
        };
        if let Some(v) = table.insert(network, geo_record) {
            return Err(anyhow!(
                "found duplicate entry for network {} as {:?}/{:?}/{:?}",
                v.network,
                v.as_number,
                v.as_name,
                v.as_domain
            ));
        }
    }

    Ok(table)
}
