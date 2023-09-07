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

use clap::{value_parser, Arg, ArgAction, Command, ValueHint};

const ARG_INPUT: &str = "input";
const ARG_IP_LIST: &str = "ip-list";

fn build_cli_args() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .arg(
            Arg::new(ARG_INPUT)
                .help("Input csv data file")
                .long(ARG_INPUT)
                .short('i')
                .num_args(1)
                .required(true)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
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

    let input = args.get_one::<PathBuf>(ARG_INPUT).unwrap();
    println!("# loading geoip data");
    let geoip_table = g3_geoip::csv::ipinfo::load_country(input)?;
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
