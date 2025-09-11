/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

use g3_geoip_types::IsoCountryCode;

use crate::{GeoIpAsnRecord, GeoIpCountryRecord};

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

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use g3_geoip_types::ContinentCode;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &str, extension: &str) -> NamedTempFile {
        let mut file = NamedTempFile::with_suffix(extension).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    fn create_temp_gz_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::with_suffix(".gz").unwrap();
        let mut encoder = GzEncoder::new(&mut file, Compression::default());
        encoder.write_all(content.as_bytes()).unwrap();
        encoder.finish().unwrap();
        file.flush().unwrap();
        file
    }

    fn valid_country_csv() -> &'static str {
        "# This is a comment\n\
         \n\
         192.168.1.0/24,US\n\
         # Another comment\n\
         10.0.0.0/8,CN\n\
         \n"
    }

    fn valid_asn_csv() -> &'static str {
        "This is a comment\n\
         \n\
         192.168.1.0/24,64512\n\
         # Another comment\n\
         10.0.0.0/8,65001\n\
         \n"
    }

    #[test]
    fn load_country_from_regular_file() {
        // valid country csv
        let file = create_temp_file(valid_country_csv(), ".csv");
        let result = load_country(file.path()).unwrap();

        assert!(!result.is_empty());
        let us_ip: IpAddr = "192.168.1.1".parse().unwrap();
        let record = result.longest_match(us_ip).unwrap();
        assert_eq!(record.1.country, IsoCountryCode::US);
        assert_eq!(record.1.continent, ContinentCode::NA);
        let cn_ip: IpAddr = "10.0.0.1".parse().unwrap();
        let record = result.longest_match(cn_ip).unwrap();
        assert_eq!(record.1.country, IsoCountryCode::CN);
        assert_eq!(record.1.continent, ContinentCode::AS);

        // invalid format csv
        let file = create_temp_file(
            "192.168.1.0/24\n\
         no_comma_separator\n\
         192.168.2.0/24,US",
            ".csv",
        );
        let result = load_country(file.path()).unwrap();

        let ip: IpAddr = "192.168.2.1".parse().unwrap();
        assert!(result.longest_match(ip).is_some());

        // invalid network address
        let file = create_temp_file("invalid_ip/24,US", ".csv");
        assert!(load_country(file.path()).is_err());

        // invalid network mask
        let file = create_temp_file("192.168.1.0/invalid_mask,US", ".csv");
        assert!(load_country(file.path()).is_err());

        // invalid country code
        let file = create_temp_file("192.168.1.0/24,XYZ", ".csv");
        assert!(load_country(file.path()).is_err());

        // file without extension
        let file = create_temp_file(valid_country_csv(), "");
        assert!(!load_country(file.path()).unwrap().is_empty());

        // file with unknown extension
        let file = create_temp_file(valid_country_csv(), ".txt");
        assert!(!load_country(file.path()).unwrap().is_empty());
    }

    #[test]
    fn load_country_from_gzip_file() {
        let file = create_temp_gz_file(valid_country_csv());
        let result = load_country(file.path()).unwrap();

        assert!(!result.is_empty());
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        assert!(result.longest_match(ip).is_some());
    }

    #[test]
    fn load_country_file_not_found() {
        let result = load_country(std::path::Path::new("/nonexistent/file.csv"));
        assert!(result.is_err());
    }

    #[test]
    fn load_asn_from_regular_file() {
        // valid asn csv
        let file = create_temp_file(valid_asn_csv(), ".csv");
        let result = load_asn(file.path()).unwrap();

        assert!(!result.is_empty());
        let ip1: IpAddr = "192.168.1.1".parse().unwrap();
        let record = result.longest_match(ip1).unwrap();
        assert_eq!(record.1.number, 64512);
        assert_eq!(record.1.name, None);
        assert_eq!(record.1.domain, None);
        let ip2: IpAddr = "10.0.0.1".parse().unwrap();
        let record = result.longest_match(ip2).unwrap();
        assert_eq!(record.1.number, 65001);

        // invalid format csv
        let file = create_temp_file(
            "192.168.1.0/24\n\
         no_comma_separator\n\
         192.168.2.0/24,65002",
            ".csv",
        );
        let result = load_asn(file.path()).unwrap();

        let ip: IpAddr = "192.168.2.1".parse().unwrap();
        assert!(result.longest_match(ip).is_some());

        // invalid network address
        let file = create_temp_file("invalid_ip/24,64512", ".csv");
        assert!(load_asn(file.path()).is_err());

        // invalid network mask
        let file = create_temp_file("192.168.1.0/invalid_mask,64512", ".csv");
        assert!(load_asn(file.path()).is_err());

        // invalid as number
        let file = create_temp_file("192.168.1.0/24,invalid_asn", ".csv");
        assert!(load_asn(file.path()).is_err());

        // file without extension
        let file = create_temp_file(valid_asn_csv(), "");
        assert!(!load_asn(file.path()).unwrap().is_empty());

        // file with unknown extension
        let file = create_temp_file(valid_asn_csv(), ".txt");
        assert!(!load_asn(file.path()).unwrap().is_empty());
    }

    #[test]
    fn load_asn_from_gzip_file() {
        let file = create_temp_gz_file(valid_asn_csv());
        let result = load_asn(file.path()).unwrap();

        assert!(!result.is_empty());
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        assert!(result.longest_match(ip).is_some());
    }

    #[test]
    fn load_asn_file_not_found() {
        let result = load_asn(std::path::Path::new("/nonexistent/file.csv"));
        assert!(result.is_err());
    }

    #[test]
    fn read_error_simulation() {
        use std::io::Error;

        struct FailingReader;
        impl std::io::Read for FailingReader {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(Error::other("simulated read error"))
            }
        }

        let result = load_country_from_csv(FailingReader);
        assert!(result.is_err());

        let result = load_asn_from_csv(FailingReader);
        assert!(result.is_err());
    }
}
