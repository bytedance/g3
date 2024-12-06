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

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};
use ip_network_table::IpNetworkTable;

use g3_geoip_db::{GeoIpAsnRecord, GeoIpCountryRecord};

const ARG_NATIVE: &str = "native";
const ARG_IPINFO: &str = "ipinfo";
const ARG_MAXMIND: &str = "maxmind";
const ARG_IPFIRE: &str = "ipfire";

const ARG_COUNTRY: &str = "country";
const ARG_ASN: &str = "asn";

const COMMAND_DUMP: &str = "dump";
const COMMAND_QUERY: &str = "query";

const ARG_IP_LIST: &str = "ip-list";
const ARG_OUTPUT: &str = "output";

fn build_cli_args() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .arg(
            Arg::new(ARG_NATIVE)
                .help("Input native data file")
                .long(ARG_NATIVE)
                .action(ArgAction::SetTrue)
                .required_unless_present_any([ARG_IPINFO, ARG_MAXMIND, ARG_IPFIRE]),
        )
        .arg(
            Arg::new(ARG_IPINFO)
                .help("Input csv data file from ipinfo.io")
                .long(ARG_IPINFO)
                .action(ArgAction::SetTrue)
                .required_unless_present_any([ARG_NATIVE, ARG_MAXMIND, ARG_IPFIRE]),
        )
        .arg(
            Arg::new(ARG_MAXMIND)
                .help("Input csv data file from maxmind.com")
                .long(ARG_MAXMIND)
                .action(ArgAction::SetTrue)
                .required_unless_present_any([ARG_NATIVE, ARG_IPINFO, ARG_IPFIRE]),
        )
        .arg(
            Arg::new(ARG_IPFIRE)
                .help("Input dump data file from ipfire")
                .long(ARG_IPFIRE)
                .action(ArgAction::SetTrue)
                .required_unless_present_any([ARG_NATIVE, ARG_IPINFO, ARG_MAXMIND]),
        )
        .arg(
            Arg::new(ARG_COUNTRY)
                .help("Set the input country db file")
                .long(ARG_COUNTRY)
                .num_args(1)
                .required_unless_present(ARG_ASN)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new(ARG_ASN)
                .help("Set the input asn db file")
                .long(ARG_ASN)
                .num_args(1)
                .required_unless_present(ARG_COUNTRY)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_QUERY).arg(
                Arg::new(ARG_IP_LIST)
                    .help("IP list")
                    .required(true)
                    .action(ArgAction::Append)
                    .value_parser(value_parser!(IpAddr)),
            ),
        )
        .subcommand(
            Command::new(COMMAND_DUMP).arg(
                Arg::new(ARG_OUTPUT)
                    .help("Output file")
                    .required(true)
                    .num_args(1)
                    .value_parser(value_parser!(PathBuf))
                    .value_hint(ValueHint::FilePath),
            ),
        )
}

fn main() -> anyhow::Result<()> {
    let args = build_cli_args().get_matches();

    let (command, sub_args) = args.subcommand().unwrap();
    match command {
        COMMAND_QUERY => query(&args, sub_args),
        COMMAND_DUMP => dump(&args, sub_args),
        _ => unreachable!(),
    }
}

fn load_country(
    args: &ArgMatches,
    db: &Path,
) -> anyhow::Result<IpNetworkTable<GeoIpCountryRecord>> {
    let table = if args.get_flag(ARG_NATIVE) {
        g3_geoip_db::vendor::native::load_country(db)?
    } else if args.get_flag(ARG_IPINFO) {
        g3_geoip_db::vendor::ipinfo::load_country(db)?
    } else if args.get_flag(ARG_MAXMIND) {
        g3_geoip_db::vendor::maxmind::load_country(db)?
    } else if args.get_flag(ARG_IPFIRE) {
        g3_geoip_db::vendor::ipfire::load_location(db)?.0
    } else {
        unreachable!()
    };
    Ok(table)
}

fn load_asn(args: &ArgMatches, db: &Path) -> anyhow::Result<IpNetworkTable<GeoIpAsnRecord>> {
    let table = if args.get_flag(ARG_NATIVE) {
        g3_geoip_db::vendor::native::load_asn(db)?
    } else if args.get_flag(ARG_IPINFO) {
        g3_geoip_db::vendor::ipinfo::load_asn(db)?
    } else if args.get_flag(ARG_MAXMIND) {
        g3_geoip_db::vendor::maxmind::load_asn(db)?
    } else if args.get_flag(ARG_IPFIRE) {
        g3_geoip_db::vendor::ipfire::load_location(db)?.1
    } else {
        unreachable!()
    };
    Ok(table)
}

fn query(args: &ArgMatches, sub_args: &ArgMatches) -> anyhow::Result<()> {
    if let Some(f) = args.get_one::<PathBuf>(ARG_COUNTRY) {
        query_country(args, sub_args, f)
    } else if let Some(f) = args.get_one::<PathBuf>(ARG_ASN) {
        query_asn(args, sub_args, f)
    } else {
        unreachable!()
    }
}

fn query_country(args: &ArgMatches, sub_args: &ArgMatches, db: &Path) -> anyhow::Result<()> {
    println!("# loading geoip country data");
    let geoip_table = load_country(args, db)?;
    let (v4l, v6l) = geoip_table.len();
    println!("# loaded {v4l} ipv4 records, {v6l} ipv6 records");

    for ip in sub_args.get_many::<IpAddr>(ARG_IP_LIST).unwrap() {
        println!("# check for IP {ip}");
        match geoip_table.longest_match(*ip) {
            Some((network, r)) => {
                println!(
                    "network: {}\ncountry: {}/{}",
                    network,
                    r.country.name(),
                    r.continent.name(),
                );
            }
            None => {
                println!("no record found");
            }
        }
    }

    Ok(())
}

fn query_asn(args: &ArgMatches, sub_args: &ArgMatches, db: &Path) -> anyhow::Result<()> {
    println!("# loading geoip asn data");
    let geoip_table = load_asn(args, db)?;
    let (v4l, v6l) = geoip_table.len();
    println!("# loaded {v4l} ipv4 records, {v6l} ipv6 records");

    for ip in sub_args.get_many::<IpAddr>(ARG_IP_LIST).unwrap() {
        println!("# check for IP {ip}");
        match geoip_table.longest_match(*ip) {
            Some((network, r)) => {
                print!("network: {}\nasn: {}", network, r.number);
                if let Some(name) = r.isp_name() {
                    print!("/{name}");
                }
                if let Some(domain) = r.isp_domain() {
                    print!("/{domain}");
                }
                println!();
            }
            None => {
                println!("no record found");
            }
        }
    }

    Ok(())
}

fn dump(args: &ArgMatches, sub_args: &ArgMatches) -> anyhow::Result<()> {
    if let Some(f) = args.get_one::<PathBuf>(ARG_COUNTRY) {
        dump_country(args, sub_args, f)
    } else if let Some(f) = args.get_one::<PathBuf>(ARG_ASN) {
        dump_asn(args, sub_args, f)
    } else {
        unreachable!()
    }
}

fn dump_country(args: &ArgMatches, sub_args: &ArgMatches, db: &Path) -> anyhow::Result<()> {
    println!("# loading geoip country data");
    let geoip_table = load_country(args, db)?;
    let (v4l, v6l) = geoip_table.len();
    println!("# loaded {v4l} ipv4 records, {v6l} ipv6 records");

    let p = sub_args.get_one::<PathBuf>(ARG_OUTPUT).unwrap();
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(p)?;
    let mut writer = BufWriter::new(file);
    for (net, v) in geoip_table.iter() {
        writer.write_fmt(format_args!("{net},{}\n", v.country.alpha2_code()))?;
    }
    writer.flush()?;

    Ok(())
}

fn dump_asn(args: &ArgMatches, sub_args: &ArgMatches, db: &Path) -> anyhow::Result<()> {
    println!("# loading geoip asn data");
    let geoip_table = load_asn(args, db)?;
    let (v4l, v6l) = geoip_table.len();
    println!("# loaded {v4l} ipv4 records, {v6l} ipv6 records");

    let p = sub_args.get_one::<PathBuf>(ARG_OUTPUT).unwrap();

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(p)?;
    let mut writer = BufWriter::new(file);
    for (net, v) in geoip_table.iter() {
        writer.write_fmt(format_args!("{net},{}\n", v.number))?;
    }
    writer.flush()?;

    Ok(())
}
