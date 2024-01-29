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

use std::fs::File;
use std::io::{self, BufReader};
use std::net::IpAddr;
use std::ops::BitXor;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use csv::StringRecord;
use flate2::bufread::GzDecoder;
use ip_network::{IpNetwork, Ipv4Network, Ipv6Network};
use ip_network_table::IpNetworkTable;

use crate::{ContinentCode, GeoIpAsnRecord, GeoIpCountryRecord, IsoCountryCode};

pub fn load_country(file: &Path) -> anyhow::Result<IpNetworkTable<GeoIpCountryRecord>> {
    if let Some(ext) = file.extension() {
        match ext.to_str() {
            Some("gz") => {
                let f = File::open(file)
                    .map_err(|e| anyhow!("failed to open gzip file {}: {e}", file.display()))?;
                let f = GzDecoder::new(BufReader::new(f));
                return load_country_from_csv(f).context(format!(
                    "failed to load records from gzip file {}",
                    file.display()
                ));
            }
            Some("csv") => {
                let f = File::open(file)
                    .map_err(|e| anyhow!("failed to open csv file {}: {e}", file.display()))?;
                return load_country_from_csv(f).context(format!(
                    "failed to load records from csv file {}",
                    file.display()
                ));
            }
            Some(_) => {}
            None => {}
        }
    }
    Err(anyhow!("file {} has no known extension", file.display()))
}

fn load_country_from_csv<R: io::Read>(
    stream: R,
) -> anyhow::Result<IpNetworkTable<GeoIpCountryRecord>> {
    let mut rdr = csv::Reader::from_reader(stream);
    let headers = rdr
        .headers()
        .map_err(|e| anyhow!("no csv header line found: {e}"))?;

    let mut start_ip_index = usize::MAX;
    let mut end_ip_index = usize::MAX;
    let mut country_index = usize::MAX;
    let mut continent_index = usize::MAX;
    for (column, s) in headers.iter().enumerate() {
        match s {
            "start_ip" => start_ip_index = column,
            "end_ip" => end_ip_index = column,
            "country" => country_index = column,
            "continent" => continent_index = column,
            _ => {}
        }
    }

    let mut table = IpNetworkTable::new();
    for (i, record) in rdr.records().enumerate() {
        let record = record.map_err(|e| anyhow!("invalid record {i}: {e}"))?;

        let Some(network) = parse_network(&record, start_ip_index, end_ip_index) else {
            continue;
        };

        macro_rules! get_field {
            ($field:ident, $index:expr) => {
                let Some($field) = record.get($index) else {
                    continue;
                };
            };
        }

        get_field!(country, country_index);
        let Ok(country) = IsoCountryCode::from_str(country) else {
            continue;
        };
        get_field!(continent, continent_index);
        let Ok(continent) = ContinentCode::from_str(continent) else {
            continue;
        };

        let geo_record = GeoIpCountryRecord { country, continent };
        if table.insert(network, geo_record).is_some() {
            return Err(anyhow!("found duplicate entry for network {}", network,));
        }
    }

    Ok(table)
}

pub fn load_asn(file: &Path) -> anyhow::Result<IpNetworkTable<GeoIpAsnRecord>> {
    if let Some(ext) = file.extension() {
        match ext.to_str() {
            Some("gz") => {
                let f = File::open(file)
                    .map_err(|e| anyhow!("failed to open gzip file {}: {e}", file.display()))?;
                let f = GzDecoder::new(BufReader::new(f));
                return load_asn_from_csv(f).context(format!(
                    "failed to load records from gzip file {}",
                    file.display()
                ));
            }
            Some("csv") => {
                let f = File::open(file)
                    .map_err(|e| anyhow!("failed to open csv file {}: {e}", file.display()))?;
                return load_asn_from_csv(f).context(format!(
                    "failed to load records from csv file {}",
                    file.display()
                ));
            }
            Some(_) => {}
            None => {}
        }
    }
    Err(anyhow!("file {} has no known extension", file.display()))
}

fn load_asn_from_csv<R: io::Read>(stream: R) -> anyhow::Result<IpNetworkTable<GeoIpAsnRecord>> {
    let mut table = IpNetworkTable::new();

    let mut rdr = csv::Reader::from_reader(stream);
    let headers = rdr
        .headers()
        .map_err(|e| anyhow!("no csv header line found: {e}"))?;

    let mut start_ip_index = usize::MAX;
    let mut end_ip_index = usize::MAX;
    let mut asn_index = usize::MAX;
    let mut as_name_index = usize::MAX;
    let mut as_domain_index = usize::MAX;
    for (column, s) in headers.iter().enumerate() {
        match s {
            "start_ip" => start_ip_index = column,
            "end_ip" => end_ip_index = column,
            "asn" => asn_index = column,
            "name" => as_name_index = column,
            "domain" => as_domain_index = column,
            _ => {}
        }
    }

    for (i, record) in rdr.records().enumerate() {
        let record = record.map_err(|e| anyhow!("invalid record {i}: {e}"))?;

        let Some(network) = parse_network(&record, start_ip_index, end_ip_index) else {
            continue;
        };

        let Some(asn) = record
            .get(asn_index)
            .and_then(|v| u32::from_str(v.strip_prefix("AS").unwrap_or(v)).ok())
        else {
            continue;
        };
        let as_name = record.get(as_name_index).map(|s| s.to_string());
        let as_domain = record.get(as_domain_index).map(|s| s.to_string());

        let geo_record = GeoIpAsnRecord {
            number: asn,
            name: as_name,
            domain: as_domain,
        };
        if let Some(v) = table.insert(network, geo_record) {
            return Err(anyhow!(
                "found duplicate entry for network {} as {}/{:?}/{:?}",
                network,
                v.number,
                v.name,
                v.domain
            ));
        }
    }

    Ok(table)
}

fn parse_network(
    record: &StringRecord,
    start_ip_index: usize,
    end_ip_index: usize,
) -> Option<IpNetwork> {
    macro_rules! get_ip_field {
        ($field:ident, $index:expr) => {
            let $field = record.get($index)?;
            let Ok($field) = IpAddr::from_str($field) else {
                return None;
            };
        };
    }

    get_ip_field!(start_ip, start_ip_index);
    get_ip_field!(end_ip, end_ip_index);
    match (start_ip, end_ip) {
        (IpAddr::V4(s), IpAddr::V4(e)) => {
            let si = u32::from(s);
            let ei = u32::from(e);
            let prefix = si.bitxor(&ei).leading_zeros() as u8;
            // the start ip may be not the network address with trailing zeros, so truncate here
            let Ok(v4_net) = Ipv4Network::new_truncate(s, prefix) else {
                return None;
            };
            Some(IpNetwork::V4(v4_net))
        }
        (IpAddr::V6(s), IpAddr::V6(e)) => {
            let si = u128::from(s);
            let ei = u128::from(e);
            let prefix = si.bitxor(&ei).leading_zeros() as u8;
            // the start ip may be not the network address with trailing zeros, so truncate here
            let Ok(v6_net) = Ipv6Network::new_truncate(s, prefix) else {
                return None;
            };
            Some(IpNetwork::V6(v6_net))
        }
        _ => None,
    }
}
