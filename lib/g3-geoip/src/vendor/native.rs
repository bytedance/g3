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
use std::io::{self, BufRead, BufReader};
use std::net::IpAddr;
use std::path::Path;
use std::str::FromStr;

use anyhow::anyhow;
use flate2::bufread::GzDecoder;
use ip_network::IpNetwork;
use ip_network_table::IpNetworkTable;

use crate::{GeoIpAsnRecord, GeoIpCountryRecord, IsoCountryCode};

pub fn load_country(file: &Path) -> anyhow::Result<IpNetworkTable<GeoIpCountryRecord>> {
    if let Some(ext) = file.extension() {
        match ext.to_str() {
            Some("gz") => {
                let f = File::open(file)
                    .map_err(|e| anyhow!("failed to open gzip file {}: {e}", file.display()))?;
                let f = GzDecoder::new(BufReader::new(f));
                return load_country_from_csv(f);
            }
            Some(_) => {}
            None => {}
        }
    }
    let f = File::open(file).map_err(|e| anyhow!("failed to open file {}: {e}", file.display()))?;
    load_country_from_csv(f)
}

fn load_country_from_csv<R: io::Read>(
    stream: R,
) -> anyhow::Result<IpNetworkTable<GeoIpCountryRecord>> {
    let mut table = IpNetworkTable::new();

    let reader = BufReader::new(stream);
    for (i, line) in reader.split(b'\n').enumerate() {
        let line = line.map_err(|e| anyhow!("failed to read line #{i}: {e}"))?;

        if line.is_empty() {
            continue;
        }
        if line[0] == b'#' {
            continue;
        }
        let line = std::str::from_utf8(&line).map_err(|e| anyhow!("invalid line #{i}: {e}"))?;
        let Some((n, c)) = line.split_once(',') else {
            continue;
        };
        let Some((n, p)) = n.split_once('/') else {
            continue;
        };

        let addr = IpAddr::from_str(n)
            .map_err(|e| anyhow!("invalid network address in line #{i}: {e}"))?;
        let mask =
            u8::from_str(p).map_err(|e| anyhow!("invalid network mask in line #{i}: {e}"))?;
        let network =
            IpNetwork::new(addr, mask).map_err(|e| anyhow!("invalid network in line #{i}: {e}"))?;
        let country = IsoCountryCode::from_str(c)
            .map_err(|_| anyhow!("invalid country code {c} in line #{i}"))?;

        table.insert(
            network,
            GeoIpCountryRecord {
                country,
                continent: country.continent(),
            },
        );
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
                return load_asn_from_csv(f);
            }
            Some(_) => {}
            None => {}
        }
    }
    let f = File::open(file).map_err(|e| anyhow!("failed to open file {}: {e}", file.display()))?;
    load_asn_from_csv(f)
}

fn load_asn_from_csv<R: io::Read>(stream: R) -> anyhow::Result<IpNetworkTable<GeoIpAsnRecord>> {
    let mut table = IpNetworkTable::new();

    let reader = BufReader::new(stream);
    for (i, line) in reader.split(b'\n').enumerate() {
        let line = line.map_err(|e| anyhow!("failed to read line #{i}: {e}"))?;

        if line.is_empty() {
            continue;
        }
        if line[0] == b'#' {
            continue;
        }
        let line = std::str::from_utf8(&line).map_err(|e| anyhow!("invalid line #{i}: {e}"))?;
        let Some((n, a)) = line.split_once(',') else {
            continue;
        };
        let Some((n, p)) = n.split_once('/') else {
            continue;
        };

        let addr = IpAddr::from_str(n)
            .map_err(|e| anyhow!("invalid network address in line #{i}: {e}"))?;
        let mask =
            u8::from_str(p).map_err(|e| anyhow!("invalid network mask in line #{i}: {e}"))?;
        let network =
            IpNetwork::new(addr, mask).map_err(|e| anyhow!("invalid network in line #{i}: {e}"))?;
        let asn = u32::from_str(a).map_err(|_| anyhow!("invalid as number {a} in line #{i}"))?;

        table.insert(
            network,
            GeoIpAsnRecord {
                number: asn,
                name: None,
                domain: None,
            },
        );
    }

    Ok(table)
}
