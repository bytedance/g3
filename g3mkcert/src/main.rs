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

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use clap::builder::ArgPredicate;
use clap::{value_parser, Arg, ArgAction, ArgGroup, ArgMatches, Command, ValueHint};
use clap_complete::Shell;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::SubjectAlternativeName;
use openssl::x509::{X509Name, X509};

use g3_tls_cert::builder::{
    ClientCertBuilder, IntermediateCertBuilder, MimicCertBuilder, RootCertBuilder,
    ServerCertBuilder, SubjectNameBuilder, TlcpClientEncCertBuilder, TlcpClientSignCertBuilder,
    TlcpServerEncCertBuilder, TlcpServerSignCertBuilder, TlsClientCertBuilder,
    TlsServerCertBuilder,
};
use g3_types::net::Host;

mod build;

const ARG_VERSION: &str = "version";
const ARG_COMPLETION: &str = "completion";

const ARG_ROOT: &str = "root";
const ARG_INTERMEDIATE: &str = "intermediate";
const ARG_TLS_SERVER: &str = "tls-server";
const ARG_TLS_CLIENT: &str = "tls-client";
const ARG_TLCP_SERVER_SIGN: &str = "tlcp-server-sign";
const ARG_TLCP_SERVER_ENC: &str = "tlcp-server-enc";
const ARG_TLCP_CLIENT_SIGN: &str = "tlcp-client-sign";
const ARG_TLCP_CLIENT_ENC: &str = "tlcp-client-enc";

const ARG_RSA: &str = "rsa";
const ARG_EC224: &str = "ec224";
const ARG_EC256: &str = "ec256";
const ARG_EC384: &str = "ec384";
const ARG_EC521: &str = "ec521";
const ARG_SM2: &str = "sm2";
const ARG_ED25519: &str = "ed25519";
const ARG_ED448: &str = "ed448";
const ARG_X25519: &str = "x25519";
const ARG_X448: &str = "x448";

const ARG_CA_CERT: &str = "ca-cert";
const ARG_CA_KEY: &str = "ca-key";
const ARG_MIMIC: &str = "mimic";

const ARG_COUNTRY: &str = "country";
const ARG_ORGANIZATION: &str = "organization";
const ARG_ORGANIZATION_UNIT: &str = "organization-unit";
const ARG_COMMON_NAME: &str = "common-name";

const ARG_PATH_LENGTH: &str = "path-length";
const ARG_HOST: &str = "host";

const ARG_OUTPUT_CERT: &str = "output-cert";
const ARG_OUTPUT_KEY: &str = "output-key";

const ARG_GROUP_SUBJECT: &str = "subject";
const ARG_GROUP_TYPE: &str = "type";
const ARG_GROUP_ALGORITHM: &str = "algorithm";

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "openssl-probe")]
    unsafe {
        openssl_probe::init_openssl_env_vars();
    }
    openssl::init();

    let args = build_cli_args().get_matches();

    if args.get_flag(ARG_VERSION) {
        build::print_version();
        Ok(())
    } else if let Some(shell) = args.get_one::<Shell>(ARG_COMPLETION) {
        let mut app = build_cli_args();
        let bin_name = app.get_name().to_string();
        clap_complete::generate(*shell, &mut app, bin_name, &mut std::io::stdout());
        Ok(())
    } else if args.get_flag(ARG_ROOT) {
        generate_root(args)
    } else if args.get_flag(ARG_INTERMEDIATE) {
        generate_intermediate(args)
    } else if args.get_flag(ARG_TLS_SERVER) {
        generate_tls_server(args)
    } else if args.get_flag(ARG_TLS_CLIENT) {
        generate_tls_client(args)
    } else if args.get_flag(ARG_TLCP_SERVER_SIGN) {
        generate_tlcp_server_sign(args)
    } else if args.get_flag(ARG_TLCP_SERVER_ENC) {
        generate_tlcp_server_enc(args)
    } else if args.get_flag(ARG_TLCP_CLIENT_SIGN) {
        generate_tlcp_client_sign(args)
    } else if args.get_flag(ARG_TLCP_CLIENT_ENC) {
        generate_tlcp_client_enc(args)
    } else {
        unreachable!()
    }
}

fn build_cli_args() -> Command {
    Command::new(build::PKG_NAME)
        .arg(
            Arg::new(ARG_VERSION)
                .help("Show version")
                .num_args(0)
                .long(ARG_VERSION)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_COMPLETION)
                .help("Generate shell completion script")
                .long(ARG_COMPLETION)
                .num_args(1)
                .value_name("SHELL")
                .value_parser(value_parser!(Shell)),
        )
        .arg(
            Arg::new(ARG_ROOT)
                .help("Generate self signed root CA certificate")
                .num_args(0)
                .long(ARG_ROOT)
                .action(ArgAction::SetTrue)
                .requires(ARG_COMMON_NAME),
        )
        .arg(
            Arg::new(ARG_INTERMEDIATE)
                .help("Generate intermediate CA certificate")
                .num_args(0)
                .long(ARG_INTERMEDIATE)
                .action(ArgAction::SetTrue)
                .requires(ARG_CA_CERT)
                .requires(ARG_CA_KEY)
                .requires(ARG_COMMON_NAME),
        )
        .arg(
            Arg::new(ARG_TLS_SERVER)
                .help("Generate end entity certificate for TLS server")
                .num_args(0)
                .long(ARG_TLS_SERVER)
                .action(ArgAction::SetTrue)
                .requires(ARG_CA_CERT)
                .requires(ARG_CA_KEY)
                .requires(ARG_HOST),
        )
        .arg(
            Arg::new(ARG_TLS_CLIENT)
                .help("Generate end entity certificate for TLS client")
                .num_args(0)
                .long(ARG_TLS_CLIENT)
                .action(ArgAction::SetTrue)
                .requires(ARG_CA_CERT)
                .requires(ARG_CA_KEY)
                .requires(ARG_HOST),
        )
        .arg(
            Arg::new(ARG_TLCP_SERVER_SIGN)
                .help("Generate end entity sign certificate for TLCP server")
                .num_args(0)
                .long(ARG_TLCP_SERVER_SIGN)
                .action(ArgAction::SetTrue)
                .requires(ARG_CA_CERT)
                .requires(ARG_CA_KEY)
                .requires(ARG_HOST),
        )
        .arg(
            Arg::new(ARG_TLCP_SERVER_ENC)
                .help("Generate end entity enc certificate for TLCP server")
                .num_args(0)
                .long(ARG_TLCP_SERVER_ENC)
                .action(ArgAction::SetTrue)
                .requires(ARG_CA_CERT)
                .requires(ARG_CA_KEY)
                .requires(ARG_HOST),
        )
        .arg(
            Arg::new(ARG_TLCP_CLIENT_SIGN)
                .help("Generate end entity sign certificate for TLCP client")
                .num_args(0)
                .long(ARG_TLCP_CLIENT_SIGN)
                .action(ArgAction::SetTrue)
                .requires(ARG_CA_CERT)
                .requires(ARG_CA_KEY)
                .requires(ARG_HOST),
        )
        .arg(
            Arg::new(ARG_TLCP_CLIENT_ENC)
                .help("Generate end entity enc certificate for TLCP client")
                .num_args(0)
                .long(ARG_TLCP_CLIENT_ENC)
                .action(ArgAction::SetTrue)
                .requires(ARG_CA_CERT)
                .requires(ARG_CA_KEY)
                .requires(ARG_HOST),
        )
        .group(
            ArgGroup::new(ARG_GROUP_TYPE)
                .args([
                    ARG_ROOT,
                    ARG_INTERMEDIATE,
                    ARG_MIMIC,
                    ARG_TLS_SERVER,
                    ARG_TLS_CLIENT,
                    ARG_TLCP_SERVER_SIGN,
                    ARG_TLCP_SERVER_ENC,
                    ARG_TLCP_CLIENT_SIGN,
                    ARG_TLCP_CLIENT_ENC,
                    ARG_VERSION,
                    ARG_COMPLETION,
                ])
                .required(true),
        )
        .arg(
            Arg::new(ARG_RSA)
                .help("Use RSA (Default to 2048 bits)")
                .value_name("BITS")
                .num_args(0..=1)
                .long(ARG_RSA)
                .value_parser(value_parser!(u32))
                .default_missing_value("2048"),
        )
        .arg(
            Arg::new(ARG_EC224)
                .help("Use Curve P-224")
                .num_args(0)
                .long(ARG_EC224)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_EC256)
                .help("Use Curve P-256 (Default)")
                .num_args(0)
                .long(ARG_EC256)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_EC384)
                .help("Use Curve P-384")
                .num_args(0)
                .long(ARG_EC384)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_EC521)
                .help("Use Curve P-521")
                .num_args(0)
                .long(ARG_EC521)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_SM2)
                .help("Use Curve SM2")
                .num_args(0)
                .long(ARG_SM2)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_ED25519)
                .help("Use Curve25519")
                .num_args(0)
                .long(ARG_ED25519)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_ED448)
                .help("Use Curve448")
                .num_args(0)
                .long(ARG_ED448)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_X25519)
                .help("Use X25519")
                .num_args(0)
                .long(ARG_X25519)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_X448)
                .help("Use X448")
                .num_args(0)
                .long(ARG_X448)
                .action(ArgAction::SetTrue),
        )
        .group(ArgGroup::new(ARG_GROUP_ALGORITHM).args([
            ARG_RSA,
            ARG_EC224,
            ARG_EC256,
            ARG_EC384,
            ARG_EC521,
            ARG_SM2,
            ARG_ED25519,
            ARG_ED448,
            ARG_X25519,
            ARG_X448,
        ]))
        .arg(
            Arg::new(ARG_CA_CERT)
                .help("CA Certificate file")
                .long(ARG_CA_CERT)
                .num_args(1)
                .value_name("CERT FILE")
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new(ARG_CA_KEY)
                .help("CA Private Key file")
                .long(ARG_CA_KEY)
                .num_args(1)
                .value_name("KEY FILE")
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new(ARG_MIMIC)
                .help("Mimic the input certificate")
                .num_args(1)
                .long(ARG_MIMIC)
                .value_name("CERT FILE")
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new(ARG_COUNTRY)
                .help("Set country field in subject name")
                .value_name("C")
                .long(ARG_COUNTRY)
                .num_args(1),
        )
        .arg(
            Arg::new(ARG_ORGANIZATION)
                .help("Set organization field in subject name")
                .value_name("O")
                .long(ARG_ORGANIZATION)
                .num_args(1),
        )
        .arg(
            Arg::new(ARG_ORGANIZATION_UNIT)
                .help("Set organization unit field in subject name")
                .value_name("OU")
                .long(ARG_ORGANIZATION_UNIT)
                .num_args(1),
        )
        .arg(
            Arg::new(ARG_COMMON_NAME)
                .help("Set common name field in subject name")
                .value_name("CN")
                .long(ARG_COMMON_NAME)
                .num_args(1),
        )
        .group(
            ArgGroup::new(ARG_GROUP_SUBJECT)
                .args([
                    ARG_COUNTRY,
                    ARG_ORGANIZATION,
                    ARG_ORGANIZATION_UNIT,
                    ARG_COMMON_NAME,
                ])
                .multiple(true),
        )
        .arg(
            Arg::new(ARG_HOST)
                .long(ARG_HOST)
                .action(ArgAction::Append)
                .value_parser(value_parser!(Host)),
        )
        .arg(
            Arg::new(ARG_PATH_LENGTH)
                .help("Set pathlen of BasicConstraints extension for CA certificate")
                .long(ARG_PATH_LENGTH)
                .num_args(1)
                .value_parser(value_parser!(u32))
                .default_value_if(ARG_INTERMEDIATE, ArgPredicate::IsPresent, "0"),
        )
        .arg(
            Arg::new(ARG_OUTPUT_CERT)
                .help("Output path for the certificate file")
                .long(ARG_OUTPUT_CERT)
                .num_args(1)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new(ARG_OUTPUT_KEY)
                .help("Output path for the private key file")
                .long(ARG_OUTPUT_KEY)
                .num_args(1)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath),
        )
}

fn get_ca_cert_and_key(args: &ArgMatches) -> anyhow::Result<(X509, PKey<Private>)> {
    let ca_cert_file = args
        .get_one::<PathBuf>(ARG_CA_CERT)
        .ok_or_else(|| anyhow!("no ca certificate set"))?;
    let ca_key_file = args
        .get_one::<PathBuf>(ARG_CA_KEY)
        .ok_or_else(|| anyhow!("no ca private key set"))?;

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

    Ok((ca_cert, ca_key))
}

fn get_mimic_cert(args: &ArgMatches) -> anyhow::Result<Option<X509>> {
    let Some(path) = args.get_one::<PathBuf>(ARG_MIMIC) else {
        return Ok(None);
    };

    let cert_content = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("failed to read mimic cert file {}: {e:?}", path.display()))?;
    let cert = X509::from_pem(cert_content.as_bytes())
        .map_err(|e| anyhow!("invalid cert in file {}: {e}", path.display()))?;
    Ok(Some(cert))
}

fn get_output_cert_file(args: &ArgMatches) -> Option<PathBuf> {
    args.get_one::<PathBuf>(ARG_OUTPUT_CERT).cloned()
}

fn get_output_key_file(args: &ArgMatches) -> Option<PathBuf> {
    args.get_one::<PathBuf>(ARG_OUTPUT_KEY).cloned()
}

fn set_subject_name(
    args: &ArgMatches,
    subject_builder: &mut SubjectNameBuilder,
) -> anyhow::Result<()> {
    if let Some(c) = args.get_one::<String>(ARG_COUNTRY) {
        subject_builder.set_country(c.to_string());
    }
    if let Some(o) = args.get_one::<String>(ARG_ORGANIZATION) {
        subject_builder.set_organization(o.to_string());
    }
    if let Some(ou) = args.get_one::<String>(ARG_ORGANIZATION_UNIT) {
        subject_builder.set_organization_unit(ou.to_string());
    }
    if let Some(cn) = args.get_one::<String>(ARG_COMMON_NAME) {
        subject_builder.set_common_name(cn.to_string());
    }
    Ok(())
}

fn get_subject_with_host(
    args: &ArgMatches,
    subject_builder: &mut SubjectNameBuilder,
) -> anyhow::Result<(X509Name, SubjectAlternativeName)> {
    set_subject_name(args, subject_builder)?;
    let mut san = SubjectAlternativeName::new();
    if let Some(hosts) = args.get_many::<Host>(ARG_HOST) {
        for host in hosts {
            match host {
                Host::Domain(domain) => {
                    subject_builder.set_common_name_if_missing(domain);
                    san.dns(domain);
                }
                Host::Ip(ip) => {
                    let text = ip.to_string();
                    subject_builder.set_common_name_if_missing(&text);
                    san.ip(&text);
                }
            }
        }
    }
    let subject_name = subject_builder.build()?;
    Ok((subject_name, san))
}

fn generate_root(args: ArgMatches) -> anyhow::Result<()> {
    let mut builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        RootCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_X448) {
        return Err(anyhow!(
            "x448 can not be used in certification authority certificate"
        ));
    } else if args.get_flag(ARG_X25519) {
        return Err(anyhow!(
            "x25519 can not be used in certification authority certificate"
        ));
    } else if args.get_flag(ARG_ED448) {
        RootCertBuilder::new_ed448()?
    } else if args.get_flag(ARG_ED25519) {
        RootCertBuilder::new_ed25519()?
    } else if args.get_flag(ARG_SM2) {
        RootCertBuilder::new_sm2()?
    } else if args.get_flag(ARG_EC521) {
        RootCertBuilder::new_ec521()?
    } else if args.get_flag(ARG_EC384) {
        RootCertBuilder::new_ec384()?
    } else if args.get_flag(ARG_EC256) {
        RootCertBuilder::new_ec256()?
    } else if args.get_flag(ARG_EC224) {
        RootCertBuilder::new_ec224()?
    } else {
        RootCertBuilder::new_ec256()?
    };

    set_subject_name(&args, builder.subject_builder_mut())?;
    let cn = builder
        .subject_builder()
        .common_name()
        .ok_or_else(|| anyhow!("no common name set"))?;

    let cert = builder.build(None)?;
    let cert_output = get_output_cert_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}.crt")));
    write_certificate_file(&cert, cert_output)?;
    let key_output = get_output_key_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}.key")));
    write_private_key_file(builder.pkey(), key_output)?;

    Ok(())
}

fn generate_intermediate(args: ArgMatches) -> anyhow::Result<()> {
    let mut builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        IntermediateCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_X448) {
        return Err(anyhow!(
            "x448 can not be used in certification authority certificate"
        ));
    } else if args.get_flag(ARG_X25519) {
        return Err(anyhow!(
            "x25519 can not be used in certification authority certificate"
        ));
    } else if args.get_flag(ARG_ED448) {
        IntermediateCertBuilder::new_ed448()?
    } else if args.get_flag(ARG_ED25519) {
        IntermediateCertBuilder::new_ed25519()?
    } else if args.get_flag(ARG_SM2) {
        IntermediateCertBuilder::new_sm2()?
    } else if args.get_flag(ARG_EC521) {
        IntermediateCertBuilder::new_ec521()?
    } else if args.get_flag(ARG_EC384) {
        IntermediateCertBuilder::new_ec384()?
    } else if args.get_flag(ARG_EC256) {
        IntermediateCertBuilder::new_ec256()?
    } else if args.get_flag(ARG_EC224) {
        IntermediateCertBuilder::new_ec224()?
    } else {
        IntermediateCertBuilder::new_ec256()?
    };

    let (ca_cert, ca_key) = get_ca_cert_and_key(&args)?;
    set_subject_name(&args, builder.subject_builder_mut())?;
    let path_len = args.get_one::<u32>(ARG_PATH_LENGTH).copied();
    let cn = builder
        .subject_builder()
        .common_name()
        .ok_or_else(|| anyhow!("no common name set"))?;

    let cert = builder.build(path_len, &ca_cert, &ca_key, None)?;
    let cert_output = get_output_cert_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}.crt")));
    write_certificate_file(&cert, cert_output)?;
    let key_output = get_output_key_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}.key")));
    write_private_key_file(builder.pkey(), key_output)?;

    Ok(())
}

fn generate_tls_server(args: ArgMatches) -> anyhow::Result<()> {
    if let Some(cert) = get_mimic_cert(&args)? {
        return generate_tls_mimic(cert, args);
    }

    let builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        TlsServerCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_X448) {
        TlsServerCertBuilder::new_x448()?
    } else if args.get_flag(ARG_X25519) {
        TlsServerCertBuilder::new_x25519()?
    } else if args.get_flag(ARG_ED448) {
        TlsServerCertBuilder::new_ed448()?
    } else if args.get_flag(ARG_ED25519) {
        TlsServerCertBuilder::new_ed25519()?
    } else if args.get_flag(ARG_SM2) {
        TlsServerCertBuilder::new_sm2()?
    } else if args.get_flag(ARG_EC521) {
        TlsServerCertBuilder::new_ec521()?
    } else if args.get_flag(ARG_EC384) {
        TlsServerCertBuilder::new_ec384()?
    } else if args.get_flag(ARG_EC256) {
        TlsServerCertBuilder::new_ec256()?
    } else if args.get_flag(ARG_EC224) {
        TlsServerCertBuilder::new_ec224()?
    } else {
        TlsServerCertBuilder::new_ec256()?
    };

    generate_server(builder, args)
}

fn generate_tlcp_server_sign(args: ArgMatches) -> anyhow::Result<()> {
    if let Some(cert) = get_mimic_cert(&args)? {
        return generate_tlcp_sign_mimic(cert, args);
    }

    let builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        TlcpServerSignCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_SM2) {
        TlcpServerSignCertBuilder::new_sm2()?
    } else if args.contains_id(ARG_GROUP_ALGORITHM) {
        return Err(anyhow!("unsupported signature algorithm"));
    } else {
        TlcpServerSignCertBuilder::new_sm2()?
    };

    generate_server(builder, args)
}

fn generate_tlcp_server_enc(args: ArgMatches) -> anyhow::Result<()> {
    if let Some(cert) = get_mimic_cert(&args)? {
        return generate_tlcp_enc_mimic(cert, args);
    }

    let builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        TlcpServerEncCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_SM2) {
        TlcpServerEncCertBuilder::new_sm2()?
    } else if args.contains_id(ARG_GROUP_ALGORITHM) {
        return Err(anyhow!("unsupported signature algorithm"));
    } else {
        TlcpServerEncCertBuilder::new_sm2()?
    };

    generate_server(builder, args)
}

fn generate_server(mut builder: ServerCertBuilder, args: ArgMatches) -> anyhow::Result<()> {
    let (ca_cert, ca_key) = get_ca_cert_and_key(&args)?;
    let (subject_name, subject_alt_name) =
        get_subject_with_host(&args, builder.subject_builder_mut())?;
    let cn = builder
        .subject_builder()
        .common_name()
        .ok_or_else(|| anyhow!("no common name set"))?;

    let cert = builder
        .build_with_subject(&subject_name, subject_alt_name, &ca_cert, &ca_key, None)
        .context("failed to build tls server certificate")?;
    let cert_output = get_output_cert_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}.crt")));
    write_certificate_file(&cert, cert_output)?;
    let key_output = get_output_key_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}.key")));
    write_private_key_file(builder.pkey(), key_output)?;

    Ok(())
}

fn generate_tls_client(args: ArgMatches) -> anyhow::Result<()> {
    if let Some(cert) = get_mimic_cert(&args)? {
        return generate_tls_mimic(cert, args);
    }

    let builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        TlsClientCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_X448) {
        TlsClientCertBuilder::new_x448()?
    } else if args.get_flag(ARG_X25519) {
        TlsClientCertBuilder::new_x25519()?
    } else if args.get_flag(ARG_ED448) {
        TlsClientCertBuilder::new_ed448()?
    } else if args.get_flag(ARG_ED25519) {
        TlsClientCertBuilder::new_ed25519()?
    } else if args.get_flag(ARG_SM2) {
        TlsClientCertBuilder::new_sm2()?
    } else if args.get_flag(ARG_EC521) {
        TlsClientCertBuilder::new_ec521()?
    } else if args.get_flag(ARG_EC384) {
        TlsClientCertBuilder::new_ec384()?
    } else if args.get_flag(ARG_EC256) {
        TlsClientCertBuilder::new_ec256()?
    } else if args.get_flag(ARG_EC224) {
        TlsClientCertBuilder::new_ec224()?
    } else {
        TlsClientCertBuilder::new_ec256()?
    };

    generate_client(builder, args)
}

fn generate_tlcp_client_sign(args: ArgMatches) -> anyhow::Result<()> {
    if let Some(cert) = get_mimic_cert(&args)? {
        return generate_tlcp_sign_mimic(cert, args);
    }

    let builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        TlcpClientSignCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_SM2) {
        TlcpClientSignCertBuilder::new_sm2()?
    } else if args.contains_id(ARG_GROUP_ALGORITHM) {
        return Err(anyhow!("unsupported signature algorithm"));
    } else {
        TlcpClientSignCertBuilder::new_sm2()?
    };

    generate_client(builder, args)
}

fn generate_tlcp_client_enc(args: ArgMatches) -> anyhow::Result<()> {
    if let Some(cert) = get_mimic_cert(&args)? {
        return generate_tlcp_enc_mimic(cert, args);
    }

    let builder = if let Some(bits) = args.get_one::<u32>(ARG_RSA) {
        TlcpClientEncCertBuilder::new_rsa(*bits)?
    } else if args.get_flag(ARG_SM2) {
        TlcpClientEncCertBuilder::new_sm2()?
    } else if args.contains_id(ARG_GROUP_ALGORITHM) {
        return Err(anyhow!("unsupported signature algorithm"));
    } else {
        TlcpClientEncCertBuilder::new_sm2()?
    };

    generate_client(builder, args)
}

fn generate_client(mut builder: ClientCertBuilder, args: ArgMatches) -> anyhow::Result<()> {
    let (ca_cert, ca_key) = get_ca_cert_and_key(&args)?;
    let (subject_name, subject_alt_name) =
        get_subject_with_host(&args, builder.subject_builder_mut())?;
    let cn = builder
        .subject_builder()
        .common_name()
        .ok_or_else(|| anyhow!("no common name set"))?;

    let cert = builder
        .build_with_subject(&subject_name, subject_alt_name, &ca_cert, &ca_key, None)
        .context("failed to build tls client certificate")?;
    let cert_output =
        get_output_cert_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}-client.crt")));
    write_certificate_file(&cert, cert_output)?;
    let key_output =
        get_output_key_file(&args).unwrap_or_else(|| cn2fn(format!("{cn}-client.key")));
    write_private_key_file(builder.pkey(), key_output)?;
    Ok(())
}

fn generate_tls_mimic(mimic_cert: X509, args: ArgMatches) -> anyhow::Result<()> {
    let (ca_cert, ca_key) = get_ca_cert_and_key(&args)?;
    let builder = MimicCertBuilder::new(&mimic_cert)?;
    let cert = builder
        .build_tls_cert(&ca_cert, &ca_key, None)
        .context("failed to build tls certificate")?;

    let cert_output =
        get_output_cert_file(&args).unwrap_or_else(|| cn2fn("tls_mimic.crt".to_string()));
    write_certificate_file(&cert, cert_output)?;

    Ok(())
}

fn generate_tlcp_enc_mimic(mimic_cert: X509, args: ArgMatches) -> anyhow::Result<()> {
    let (ca_cert, ca_key) = get_ca_cert_and_key(&args)?;
    let builder = MimicCertBuilder::new(&mimic_cert)?;
    let cert = builder
        .build_tlcp_enc_cert(&ca_cert, &ca_key, None)
        .context("failed to build tls certificate")?;

    let cert_output =
        get_output_cert_file(&args).unwrap_or_else(|| cn2fn("tlcp_en_mimic.crt".to_string()));
    write_certificate_file(&cert, cert_output)?;

    Ok(())
}

fn generate_tlcp_sign_mimic(mimic_cert: X509, args: ArgMatches) -> anyhow::Result<()> {
    let (ca_cert, ca_key) = get_ca_cert_and_key(&args)?;
    let builder = MimicCertBuilder::new(&mimic_cert)?;
    let cert = builder
        .build_tlcp_sign_cert(&ca_cert, &ca_key, None)
        .context("failed to build tls certificate")?;

    let cert_output =
        get_output_cert_file(&args).unwrap_or_else(|| cn2fn("tlcp_sign_mimic.crt".to_string()));
    write_certificate_file(&cert, cert_output)?;

    Ok(())
}

fn cn2fn(s: String) -> PathBuf {
    let n = s.split_whitespace().collect::<Vec<&str>>().join("_");
    PathBuf::from(n)
}

fn write_certificate_file<P: AsRef<Path>>(cert: &X509, path: P) -> anyhow::Result<()> {
    let content = cert
        .to_pem()
        .map_err(|e| anyhow!("failed to encode certificate: {e}"))?;
    let mut cert_file = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path.as_ref())
        .map_err(|e| {
            anyhow!(
                "failed to open cert output file {}: {e:?}",
                path.as_ref().display()
            )
        })?;
    cert_file.write_all(&content).map_err(|e| {
        anyhow!(
            "failed to write certificate to file {}: {e:?}",
            path.as_ref().display()
        )
    })?;
    println!("certificate saved to {}", path.as_ref().display());
    Ok(())
}

fn write_private_key_file<P: AsRef<Path>>(key: &PKey<Private>, path: P) -> anyhow::Result<()> {
    let content = key
        .private_key_to_pem_pkcs8()
        .map_err(|e| anyhow!("failed to encode private key: {e}"))?;
    let mut key_file = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path.as_ref())
        .map_err(|e| {
            anyhow!(
                "failed to open private key output file {}: {e:?}",
                path.as_ref().display()
            )
        })?;
    key_file.write_all(&content).map_err(|e| {
        anyhow!(
            "failed to write private key to file {}: {e:?}",
            path.as_ref().display()
        )
    })?;
    println!("private key saved to {}", path.as_ref().display());
    Ok(())
}
