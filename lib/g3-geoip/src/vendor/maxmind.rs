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

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use ip_network::IpNetwork;
use ip_network_table::IpNetworkTable;
use zip::ZipArchive;

use crate::{ContinentCode, GeoIpAsnRecord, GeoIpCountryRecord, IsoCountryCode};

const GEOLITE2_COUNTRY_LOCATIONS: &str = "GeoLite2-Country-Locations-en.csv";
const GEOLITE2_COUNTRY_V4: &str = "GeoLite2-Country-Blocks-IPv4.csv";
const GEOLITE2_COUNTRY_V6: &str = "GeoLite2-Country-Blocks-IPv6.csv";
const GEOLITE2_ASN_V4: &str = "GeoLite2-ASN-Blocks-IPv4.csv";
const GEOLITE2_ASN_V6: &str = "GeoLite2-ASN-Blocks-IPv6.csv";

pub fn load_country(file: &Path) -> anyhow::Result<IpNetworkTable<GeoIpCountryRecord>> {
    if let Some(ext) = file.extension() {
        match ext.to_str() {
            Some("zip") => {
                let f = File::open(file)
                    .map_err(|e| anyhow!("failed to open zip file {}: {e}", file.display()))?;
                return load_country_from_zip(f).context(format!(
                    "failed to read records from file {}",
                    file.display()
                ));
            }
            Some(_) => {}
            None => {}
        }
    }
    Err(anyhow!("file {} has no known extension", file.display()))
}

macro_rules! zip_find_file {
    ($z:ident, $v:ident, $name:expr) => {
        let file_name = $z
            .file_names()
            .find(|v| v.ends_with($name))
            .ok_or_else(|| anyhow!("no entry with name {} found", $name))?
            .to_string();
        let $v = $z
            .by_name(&file_name)
            .map_err(|e| anyhow!("{}: {e}", $name))?;
    };
}

fn load_country_from_zip<R: io::Read + io::Seek>(
    stream: R,
) -> anyhow::Result<IpNetworkTable<GeoIpCountryRecord>> {
    let mut zip =
        ZipArchive::new(stream).map_err(|e| anyhow!("failed to open zip archive: {e}"))?;
    zip_find_file!(zip, locations_csv, GEOLITE2_COUNTRY_LOCATIONS);
    let locations_map = load_country_location_map_from_csv(locations_csv)
        .context(format!("failed to parse {GEOLITE2_COUNTRY_LOCATIONS}"))?;

    let mut table = IpNetworkTable::new();

    zip_find_file!(zip, v4_csv, GEOLITE2_COUNTRY_V4);
    load_country_blocks_from_csv(v4_csv, &locations_map, &mut table)
        .context(format!("failed to parse records in {GEOLITE2_COUNTRY_V4}"))?;

    zip_find_file!(zip, v6_csv, GEOLITE2_COUNTRY_V6);
    load_country_blocks_from_csv(v6_csv, &locations_map, &mut table)
        .context(format!("failed to parse records in {GEOLITE2_COUNTRY_V6}"))?;

    Ok(table)
}

fn load_country_location_map_from_csv<R: io::Read>(
    stream: R,
) -> anyhow::Result<HashMap<u32, (IsoCountryCode, ContinentCode)>> {
    let mut rdr = csv::Reader::from_reader(stream);
    let headers = rdr
        .headers()
        .map_err(|e| anyhow!("no csv header line found: {e}"))?;

    let mut geoname_id_index = usize::MAX;
    let mut country_index = usize::MAX;
    let mut continent_index = usize::MAX;
    for (column, s) in headers.iter().enumerate() {
        match s {
            "geoname_id" => geoname_id_index = column,
            "country_iso_code" => country_index = column,
            "continent_code" => continent_index = column,
            _ => {}
        }
    }

    let mut table = HashMap::new();
    for (i, record) in rdr.records().enumerate() {
        let record = record.map_err(|e| anyhow!("invalid record {i}: {e}"))?;

        macro_rules! get_field {
            ($field:ident, $index:expr, $vtype:ty) => {
                let Some($field) = record.get($index) else {
                    continue;
                };
                let Ok($field) = <$vtype>::from_str($field) else {
                    continue;
                };
            };
        }

        get_field!(geoname_id, geoname_id_index, u32);
        get_field!(country, country_index, IsoCountryCode);
        get_field!(continent, continent_index, ContinentCode);

        table.insert(geoname_id, (country, continent));
    }
    Ok(table)
}

fn load_country_blocks_from_csv<R: io::Read>(
    stream: R,
    locations_map: &HashMap<u32, (IsoCountryCode, ContinentCode)>,
    table: &mut IpNetworkTable<GeoIpCountryRecord>,
) -> anyhow::Result<()> {
    let mut rdr = csv::Reader::from_reader(stream);
    let headers = rdr
        .headers()
        .map_err(|e| anyhow!("no csv header line found: {e}"))?;

    let mut network_index = usize::MAX;
    let mut geoname_id_index = usize::MAX;
    let mut registered_country_geoname_id_index = usize::MAX;
    for (column, s) in headers.iter().enumerate() {
        match s {
            "network" => network_index = column,
            "geoname_id" => geoname_id_index = column,
            "registered_country_geoname_id" => registered_country_geoname_id_index = column,
            _ => {}
        }
    }

    for (i, record) in rdr.records().enumerate() {
        let record = record.map_err(|e| anyhow!("invalid record {i}: {e}"))?;

        let Some(network) = record
            .get(network_index)
            .and_then(|v| IpNetwork::from_str(v).ok())
        else {
            continue;
        };
        let mut id_str = record.get(geoname_id_index).unwrap_or_default();
        if id_str.is_empty() {
            id_str = record
                .get(registered_country_geoname_id_index)
                .unwrap_or_default();
        }
        let geoname_id = u32::from_str(id_str)
            .map_err(|e| anyhow!("invalid geoname_id value for record {i}: {e}"))?;

        if let Some(v) = locations_map.get(&geoname_id) {
            table.insert(
                network,
                GeoIpCountryRecord {
                    network,
                    country: v.0,
                    continent: v.1,
                },
            );
        }
    }

    Ok(())
}

pub fn load_asn(file: &Path) -> anyhow::Result<IpNetworkTable<GeoIpAsnRecord>> {
    if let Some(ext) = file.extension() {
        match ext.to_str() {
            Some("zip") => {
                let f = File::open(file)
                    .map_err(|e| anyhow!("failed to open zip file {}: {e}", file.display()))?;
                return load_asn_from_zip(f).context(format!(
                    "failed to read records from file {}",
                    file.display()
                ));
            }
            Some(_) => {}
            None => {}
        }
    }
    Err(anyhow!("file {} has no known extension", file.display()))
}

fn load_asn_from_zip<R: io::Read + io::Seek>(
    stream: R,
) -> anyhow::Result<IpNetworkTable<GeoIpAsnRecord>> {
    let mut zip =
        ZipArchive::new(stream).map_err(|e| anyhow!("failed to open zip archive: {e}"))?;

    let mut table = IpNetworkTable::new();

    zip_find_file!(zip, v4_csv, GEOLITE2_ASN_V4);
    load_asn_blocks_from_csv(v4_csv, &mut table)
        .context(format!("failed to parse records in {GEOLITE2_ASN_V4}"))?;

    zip_find_file!(zip, v6_csv, GEOLITE2_ASN_V6);
    load_asn_blocks_from_csv(v6_csv, &mut table)
        .context(format!("failed to parse records in {GEOLITE2_ASN_V6}"))?;

    Ok(table)
}

fn load_asn_blocks_from_csv<R: io::Read>(
    stream: R,
    table: &mut IpNetworkTable<GeoIpAsnRecord>,
) -> anyhow::Result<()> {
    let mut rdr = csv::Reader::from_reader(stream);
    let headers = rdr
        .headers()
        .map_err(|e| anyhow!("no csv header line found: {e}"))?;

    let mut network_index = usize::MAX;
    let mut asn_index = usize::MAX;
    let mut as_name_index = usize::MAX;
    for (column, s) in headers.iter().enumerate() {
        match s {
            "network" => network_index = column,
            "autonomous_system_number" => asn_index = column,
            "autonomous_system_organization" => as_name_index = column,
            _ => {}
        }
    }

    for (i, record) in rdr.records().enumerate() {
        let record = record.map_err(|e| anyhow!("invalid record {i}: {e}"))?;

        let Some(network) = record
            .get(network_index)
            .and_then(|v| IpNetwork::from_str(v).ok())
        else {
            continue;
        };
        let Some(asn) = record.get(asn_index).and_then(|v| u32::from_str(v).ok()) else {
            continue;
        };
        let as_name = record.get(as_name_index).map(|s| s.to_string());

        table.insert(
            network,
            GeoIpAsnRecord {
                network,
                number: asn,
                name: as_name,
                domain: None,
            },
        );
    }

    Ok(())
}
