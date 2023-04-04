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

use anyhow::{anyhow, Context};
use std::io::Write;
use std::path::PathBuf;

use clap::{value_parser, Arg, ArgAction, Command};
use openssl::pkey::{PKey, Private};
use openssl::x509::{X509Ref, X509};

use g3_tls_cert::builder::ServerCertBuilder;
use g3_types::net::Host;

const ARG_CA_CERT: &str = "ca-cert";
const ARG_CA_KEY: &str = "ca-key";
const ARG_HOST: &str = "host";

fn main() -> anyhow::Result<()> {
    let args = Command::new("g3fcgen-one")
        .arg(
            Arg::new(ARG_CA_CERT)
                .help("CA Certificate file")
                .long(ARG_CA_CERT)
                .num_args(1)
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new(ARG_CA_KEY)
                .help("CA Private Key file")
                .long(ARG_CA_KEY)
                .num_args(1)
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new(ARG_HOST)
                .action(ArgAction::Append)
                .required(true)
                .value_parser(value_parser!(Host)),
        )
        .get_matches();

    let builder = ServerCertBuilder::new()?;

    let ca_cert_file = args.get_one::<PathBuf>(ARG_CA_CERT).unwrap();
    let ca_key_file = args.get_one::<PathBuf>(ARG_CA_KEY).unwrap();

    let ca_cert_content = std::fs::read_to_string(ca_cert_file).map_err(|e| {
        anyhow!(
            "failed to read ca cert file {}: {e:?}",
            ca_cert_file.display()
        )
    })?;
    let ca_cert = X509::from_pem(ca_cert_content.as_bytes())
        .map_err(|e| anyhow!("invalid ca cert in file {}: {e}", ca_cert_file.display()))?;

    let ca_key_content = std::fs::read_to_string(ca_key_file).map_err(|e| {
        anyhow!(
            "failed to read ca pkey file {}: {e:?}",
            ca_key_file.display()
        )
    })?;
    let ca_key = PKey::private_key_from_pem(ca_key_content.as_bytes())
        .map_err(|e| anyhow!("invalid ca pkey in file {}: {e}", ca_key_file.display()))?;

    let hosts = args.get_many::<Host>(ARG_HOST).unwrap();

    for host in hosts {
        if let Err(e) = generate_one(&builder, host, &ca_cert, &ca_key) {
            eprintln!("== {host}:\n {e:?}");
        }
    }

    Ok(())
}

fn generate_one(
    builder: &ServerCertBuilder,
    host: &Host,
    ca_cert: &X509Ref,
    ca_key: &PKey<Private>,
) -> anyhow::Result<()> {
    let cert = builder
        .build_fake(host, ca_cert, ca_key)
        .context("failed to build fake certificate")?;
    let cert = cert
        .to_pem()
        .map_err(|e| anyhow!("failed to encode certificate: {e}"))?;
    let key = builder
        .pkey()
        .private_key_to_pem_pkcs8()
        .map_err(|e| anyhow!("failed to encode pkey: {e}"))?;

    let cert_output_file = format!("{host}.pem");
    let mut cert_file = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&cert_output_file)
        .map_err(|e| anyhow!("failed to open cert output file {cert_output_file}: {e:?}"))?;
    cert_file
        .write_all(&cert)
        .map_err(|e| anyhow!("failed to write cert to file {cert_output_file}: {e:?}"))?;

    let key_output_file = format!("{host}-key.pem");
    let mut key_file = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&key_output_file)
        .map_err(|e| anyhow!("failed to open pkey output file {key_output_file}: {e:?}"))?;
    key_file
        .write_all(&key)
        .map_err(|e| anyhow!("failed to write pkey to file {key_output_file}: {e:?}"))?;
    Ok(())
}
