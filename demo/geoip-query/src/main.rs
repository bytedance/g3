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
use std::path::PathBuf;

use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};

const ARG_IPINFO: &str = "ipinfo";
const ARG_MAXMIND: &str = "maxmind";

const ARG_COUNTRY: &str = "country";
const ARG_ASN: &str = "asn";

const ARG_IP_LIST: &str = "ip-list";

fn build_cli_args() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .arg(
            Arg::new(ARG_IPINFO)
                .help("Input csv data file from ipinfo.io")
                .long(ARG_IPINFO)
                .num_args(1)
                .required_unless_present(ARG_MAXMIND)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new(ARG_MAXMIND)
                .help("Input csv data file from maxmind.com")
                .long(ARG_MAXMIND)
                .num_args(1)
                .required_unless_present(ARG_IPINFO)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new(ARG_COUNTRY)
                .long(ARG_COUNTRY)
                .action(ArgAction::SetTrue)
                .required_unless_present(ARG_ASN),
        )
        .arg(
            Arg::new(ARG_ASN)
                .long(ARG_ASN)
                .action(ArgAction::SetTrue)
                .required_unless_present(ARG_COUNTRY),
        )
        .arg(
            Arg::new(ARG_IP_LIST)
                .action(ArgAction::Append)
                .required(true)
                .value_parser(value_parser!(IpAddr)),
        )
}

fn main() -> anyhow::Result<()> {
    let args = build_cli_args().get_matches();

    if args.get_flag(ARG_COUNTRY) {
        query_country(&args)
    } else if args.get_flag(ARG_ASN) {
        query_asn(&args)
    } else {
        unreachable!()
    }
}

fn query_country(args: &ArgMatches) -> anyhow::Result<()> {
    println!("# loading geoip country data");
    let geoip_table = if let Some(v) = args.get_one::<PathBuf>(ARG_IPINFO) {
        g3_geoip::csv::ipinfo::load_country(v)?
    } else if let Some(v) = args.get_one::<PathBuf>(ARG_MAXMIND) {
        g3_geoip::csv::maxmind::load_country(v)?
    } else {
        unreachable!()
    };
    let (v4l, v6l) = geoip_table.len();
    println!("# loaded {v4l} ipv4 records, {v6l} ipv6 records");

    for ip in args.get_many::<IpAddr>(ARG_IP_LIST).unwrap() {
        println!("# check for IP {ip}");
        match geoip_table.longest_match(*ip) {
            Some((_, r)) => {
                println!(
                    "network: {}\ncountry: {}/{}",
                    r.network,
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

fn query_asn(args: &ArgMatches) -> anyhow::Result<()> {
    println!("# loading geoip asn data");
    let geoip_table = if let Some(v) = args.get_one::<PathBuf>(ARG_IPINFO) {
        g3_geoip::csv::ipinfo::load_asn(v)?
    } else if let Some(v) = args.get_one::<PathBuf>(ARG_MAXMIND) {
        g3_geoip::csv::maxmind::load_asn(v)?
    } else {
        unreachable!()
    };
    let (v4l, v6l) = geoip_table.len();
    println!("# loaded {v4l} ipv4 records, {v6l} ipv6 records");

    for ip in args.get_many::<IpAddr>(ARG_IP_LIST).unwrap() {
        println!("# check for IP {ip}");
        match geoip_table.longest_match(*ip) {
            Some((_, r)) => {
                print!("network: {}\nasn: {}", r.network, r.number,);
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
