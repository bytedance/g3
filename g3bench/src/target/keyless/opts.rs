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
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, ArgGroup, ArgMatches, Command};
use openssl::x509::X509;

const ARG_CERT: &str = "cert";
const ARG_RSA_DECRYPT: &str = "rsa-decrypt";
const ARG_RSA_DECRYPT_RAW: &str = "rsa-decrypt-raw";
const ARG_RSA_SIGN: &str = "rsa-sign";
const ARG_ECDSA_SIGN: &str = "ecdsa-sign";
const ARG_DIGEST_TYPE: &str = "digest-type";

const DIGEST_TYPES: [&str; 6] = ["md5sha1", "sha1", "sha224", "sha256", "sha384", "sha512"];

#[derive(Clone, Copy, Debug)]
pub(crate) enum KeylessSignDigest {
    Md5Sha1,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

impl FromStr for KeylessSignDigest {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "md5sha1" => Ok(KeylessSignDigest::Md5Sha1),
            "sha1" => Ok(KeylessSignDigest::Sha1),
            "sha224" => Ok(KeylessSignDigest::Sha224),
            "sha256" => Ok(KeylessSignDigest::Sha256),
            "sha384" => Ok(KeylessSignDigest::Sha384),
            "sha512" => Ok(KeylessSignDigest::Sha512),
            _ => Err(anyhow!("unsupported digest type {s}")),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum KeylessAction {
    RsaDecrypt,
    RsaDecryptRaw,
    RsaSign(KeylessSignDigest),
    EcdsaSign(KeylessSignDigest),
}

pub(super) trait AppendKeylessArgs {
    fn append_keyless_args(self) -> Self;
}

pub(super) struct KeylessGlobalArgs {
    pub(super) cert: X509,
    pub(super) action: KeylessAction,
    pub(super) payload: Vec<u8>,
}

impl KeylessGlobalArgs {
    pub(super) fn parse_args(args: &ArgMatches) -> anyhow::Result<Self> {
        let Some(file) = args.get_one::<PathBuf>(ARG_CERT) else {
            unreachable!();
        };
        let cert = load_cert(file).context(format!(
            "failed to load client certificate from file {}",
            file.display()
        ))?;

        let action = if args.contains_id(ARG_RSA_DECRYPT) {
            KeylessAction::RsaDecrypt
        } else if args.contains_id(ARG_RSA_DECRYPT_RAW) {
            KeylessAction::RsaDecryptRaw
        } else if args.contains_id(ARG_RSA_SIGN) {
            let digest_str = args.get_one::<String>(ARG_DIGEST_TYPE).unwrap();
            let digest_type = KeylessSignDigest::from_str(digest_str)?;
            KeylessAction::RsaSign(digest_type)
        } else if args.contains_id(ARG_ECDSA_SIGN) {
            let digest_str = args.get_one::<String>(ARG_DIGEST_TYPE).unwrap();
            let digest_type = KeylessSignDigest::from_str(digest_str)?;
            KeylessAction::EcdsaSign(digest_type)
        } else {
            return Err(anyhow!("no keyless action set"));
        };

        Ok(KeylessGlobalArgs {
            cert,
            action,
            payload: Vec::new(),
        })
    }
}

fn load_cert(path: &Path) -> anyhow::Result<X509> {
    const MAX_FILE_SIZE: usize = 4_000_000; // 4MB
    let mut contents = String::with_capacity(MAX_FILE_SIZE);
    let file =
        File::open(path).map_err(|e| anyhow!("unable to open file {}: {e}", path.display()))?;
    file.take(MAX_FILE_SIZE as u64)
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("failed to read contents of file {}: {e}", path.display()))?;
    let mut certs = X509::stack_from_pem(contents.as_bytes())
        .map_err(|e| anyhow!("invalid certificate file({}): {e}", path.display()))?;
    if certs.is_empty() {
        Err(anyhow!(
            "no valid certificate found in file {}",
            path.display()
        ))
    } else {
        Ok(certs.pop().unwrap())
    }
}

fn add_keyless_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new(ARG_CERT)
            .help("Target certificate file")
            .num_args(1)
            .long(ARG_CERT)
            .value_parser(value_parser!(PathBuf))
            .required(true),
    )
    .arg(
        Arg::new(ARG_RSA_DECRYPT)
            .help("RSA Decrypt")
            .num_args(0)
            .long(ARG_RSA_DECRYPT)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_RSA_DECRYPT_RAW)
            .help("RSA Decrypt Raw")
            .num_args(0)
            .long(ARG_RSA_DECRYPT_RAW)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_RSA_SIGN)
            .help("RSA Sign")
            .num_args(0)
            .long(ARG_RSA_SIGN)
            .action(ArgAction::SetTrue)
            .requires(ARG_DIGEST_TYPE),
    )
    .arg(
        Arg::new(ARG_ECDSA_SIGN)
            .help("ECDSA Sign")
            .num_args(0)
            .long(ARG_ECDSA_SIGN)
            .action(ArgAction::SetTrue)
            .requires(ARG_DIGEST_TYPE),
    )
    .group(
        ArgGroup::new("method")
            .args([
                ARG_RSA_DECRYPT,
                ARG_RSA_DECRYPT_RAW,
                ARG_RSA_SIGN,
                ARG_ECDSA_SIGN,
            ])
            .required(true),
    )
    .arg(
        Arg::new(ARG_DIGEST_TYPE)
            .help("Sign Digest Type")
            .num_args(1)
            .long(ARG_DIGEST_TYPE)
            .value_parser(DIGEST_TYPES),
    )
}

impl AppendKeylessArgs for Command {
    fn append_keyless_args(self) -> Self {
        add_keyless_args(self)
    }
}
