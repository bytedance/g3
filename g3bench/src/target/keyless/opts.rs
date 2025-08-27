/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command, ValueHint, value_parser};
use openssl::hash::MessageDigest;
use openssl::md::{Md, MdRef};
use openssl::nid::Nid;
use openssl::pkey::{Id, PKey, Private, Public};
use openssl::pkey_ctx::PkeyCtx;
use openssl::rsa::Padding;

use g3_tls_cert::ext::PublicKeyExt;

use crate::ProcArgs;

const ARG_CERT: &str = "cert";
const ARG_PKEY: &str = "key";
const ARG_RSA_PRIVATE_ENCRYPT: &str = "rsa-private-encrypt";
const ARG_RSA_PUBLIC_DECRYPT: &str = "rsa-public-decrypt";
const ARG_SIGN: &str = "sign";
const ARG_DECRYPT: &str = "decrypt";
const ARG_ENCRYPT: &str = "encrypt";
const ARG_DIGEST_TYPE: &str = "digest-type";
const ARG_RSA_PADDING: &str = "rsa-padding";
const ARG_PAYLOAD: &str = "payload";
const ARG_DUMP_RESULT: &str = "dump-result";
const ARG_VERIFY: &str = "verify";
const ARG_VERIFY_DATA: &str = "verify-data";

const DIGEST_TYPES: [&str; 6] = ["md5sha1", "sha1", "sha224", "sha256", "sha384", "sha512"];
const RSA_PADDING_VALUES: [&str; 5] = ["PKCS1", "OAEP", "PSS", "X931", "NONE"];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum KeylessRsaPadding {
    #[default]
    Pkcs1,
    Oaep,
    Pss,
    X931,
    None,
}

impl KeylessRsaPadding {
    fn check_encrypt_payload(&self, rsa_size: usize, payload: &[u8]) -> anyhow::Result<()> {
        let reserve_size: usize = match self {
            KeylessRsaPadding::Pkcs1 => 11,
            KeylessRsaPadding::Oaep => 42,
            _ => 0,
        };
        if payload.len() + reserve_size > rsa_size {
            Err(anyhow!(
                "rsa encrypt payload length should be less than {rsa_size} - {reserve_size}"
            ))
        } else {
            Ok(())
        }
    }
}

impl FromStr for KeylessRsaPadding {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pkcs1" => Ok(KeylessRsaPadding::Pkcs1),
            "oaep" => Ok(KeylessRsaPadding::Oaep),
            "pss" => Ok(KeylessRsaPadding::Pss),
            "x931" => Ok(KeylessRsaPadding::X931),
            "none" => Ok(KeylessRsaPadding::None),
            _ => Err(anyhow!("unsupported rsa padding type {s}")),
        }
    }
}

impl From<KeylessRsaPadding> for Padding {
    fn from(value: KeylessRsaPadding) -> Self {
        match value {
            KeylessRsaPadding::None => Padding::NONE,
            KeylessRsaPadding::Pkcs1 => Padding::PKCS1,
            KeylessRsaPadding::Oaep => Padding::PKCS1_OAEP,
            KeylessRsaPadding::Pss => Padding::PKCS1_PSS,
            KeylessRsaPadding::X931 => Padding::from_raw(5),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum KeylessSignDigest {
    Md5Sha1,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

impl KeylessSignDigest {
    fn check_payload(&self, payload: &[u8]) -> anyhow::Result<()> {
        let digest_size = self.md().size();
        if digest_size != payload.len() {
            return Err(anyhow!(
                "payload size {} not match digest size {digest_size}",
                payload.len()
            ));
        }
        Ok(())
    }

    fn md(&self) -> &'static MdRef {
        match self {
            KeylessSignDigest::Md5Sha1 => Md::from_nid(Nid::MD5_SHA1).unwrap(),
            KeylessSignDigest::Sha1 => Md::sha1(),
            KeylessSignDigest::Sha224 => Md::sha224(),
            KeylessSignDigest::Sha256 => Md::sha256(),
            KeylessSignDigest::Sha384 => Md::sha384(),
            KeylessSignDigest::Sha512 => Md::sha512(),
        }
    }
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
    RsaSign(KeylessSignDigest, KeylessRsaPadding),
    EcdsaSign(KeylessSignDigest),
    Ed25519Sign,
    RsaDecrypt(KeylessRsaPadding),
    RsaEncrypt(KeylessRsaPadding),
    Encrypt,
    Decrypt,
    RsaPrivateEncrypt(KeylessRsaPadding),
    RsaPublicDecrypt(KeylessRsaPadding),
}

pub(super) trait AppendKeylessArgs {
    fn append_keyless_args(self) -> Self;
}

pub(super) struct KeylessGlobalArgs {
    public_key: PKey<Public>,
    public_key_ski: Vec<u8>,
    pub(super) private_key: Option<PKey<Private>>,
    pub(super) action: KeylessAction,
    pub(super) payload: Vec<u8>,
    dump_result: bool,
    verify: bool,
    verify_data: Vec<u8>,
}

impl KeylessGlobalArgs {
    pub(super) fn parse_args(args: &ArgMatches) -> anyhow::Result<Self> {
        let mut public_key_ski = None;

        let cert = if let Some(file) = args.get_one::<PathBuf>(ARG_CERT) {
            let cert = crate::module::openssl::load_certs(file)?
                .into_iter()
                .next()
                .unwrap();

            let ski = if let Some(o) = cert.subject_key_id() {
                o.as_slice().to_vec()
            } else {
                cert.pubkey_digest(MessageDigest::sha1())
                    .map_err(|e| anyhow!("failed to get sha1 hash of pubkey digest: {e}"))?
                    .to_vec()
            };
            public_key_ski = Some(ski);

            Some(cert)
        } else {
            None
        };

        let private_key = if let Some(file) = args.get_one::<PathBuf>(ARG_PKEY) {
            let key = crate::module::openssl::load_key(file)?;

            // verify SKI match
            let ski = key
                .ski()
                .map_err(|e| anyhow!("failed to get SKI from key file {}: {e}", file.display()))?;
            let ski = ski.to_vec();

            if let Some(ski_cert) = &public_key_ski {
                if ski.ne(ski_cert) {
                    return Err(anyhow!(
                        "the supplied certificate and private key not match"
                    ));
                }
            } else {
                public_key_ski = Some(ski);
            }

            Some(key)
        } else {
            None
        };

        let public_key = if let Some(key) = &private_key {
            let public_key_der = key
                .public_key_to_der()
                .map_err(|e| anyhow!("failed to get public key from private key: {e}"))?;
            PKey::public_key_from_der(public_key_der.as_slice())
                .map_err(|e| anyhow!("failed to build public key from private key: {e}"))?
        } else if let Some(cert) = &cert {
            cert.public_key()
                .map_err(|e| anyhow!("failed to fetch pubkey: {e}"))?
        } else {
            unreachable!()
        };
        let public_key_ski = public_key_ski.unwrap();

        let payload_str = args.get_one::<String>(ARG_PAYLOAD).unwrap();
        let payload = hex::decode(payload_str)
            .map_err(|e| anyhow!("the payload string is not valid hex string: {e}"))?;

        let rsa_padding = if let Some(s) = args.get_one::<String>(ARG_RSA_PADDING) {
            KeylessRsaPadding::from_str(s)?
        } else {
            KeylessRsaPadding::default()
        };

        let action = if args.get_flag(ARG_SIGN) {
            let digest_str = args.get_one::<String>(ARG_DIGEST_TYPE).unwrap();
            let digest_type = KeylessSignDigest::from_str(digest_str)?;

            match public_key.id() {
                Id::RSA => {
                    digest_type.check_payload(payload.as_slice())?;
                    KeylessAction::RsaSign(digest_type, rsa_padding)
                }
                Id::EC => {
                    digest_type.check_payload(payload.as_slice())?;
                    KeylessAction::EcdsaSign(digest_type)
                }
                Id::ED25519 => KeylessAction::Ed25519Sign,
                id => return Err(anyhow!("unsupported public key type {id:?}")),
            }
        } else if args.get_flag(ARG_DECRYPT) {
            match public_key.id() {
                Id::RSA => {
                    let rsa_size = public_key.rsa().unwrap().size() as usize;
                    if payload.len() < rsa_size {
                        return Err(anyhow!(
                            "payload length {} not match rsa decrypt data length {rsa_size}",
                            payload.len()
                        ));
                    }
                    KeylessAction::RsaDecrypt(rsa_padding)
                }
                _ => KeylessAction::Decrypt,
            }
        } else if args.get_flag(ARG_ENCRYPT) {
            match public_key.id() {
                Id::RSA => {
                    let rsa_size = public_key.rsa().unwrap().size() as usize;
                    rsa_padding.check_encrypt_payload(rsa_size, payload.as_slice())?;
                    KeylessAction::RsaEncrypt(rsa_padding)
                }
                _ => KeylessAction::Encrypt,
            }
        } else if args.get_flag(ARG_RSA_PRIVATE_ENCRYPT) {
            KeylessAction::RsaPrivateEncrypt(rsa_padding)
        } else if args.get_flag(ARG_RSA_PUBLIC_DECRYPT) {
            KeylessAction::RsaPublicDecrypt(rsa_padding)
        } else {
            return Err(anyhow!("no keyless action set"));
        };

        let dump_result = args.get_flag(ARG_DUMP_RESULT);
        let verify = args.get_flag(ARG_VERIFY);
        let verify_data = if let Some(s) = args.get_one::<String>(ARG_VERIFY_DATA) {
            hex::decode(s.as_bytes()).map_err(|e| anyhow!("invalid verify data: {e}"))?
        } else {
            vec![]
        };

        Ok(KeylessGlobalArgs {
            public_key,
            private_key,
            public_key_ski,
            action,
            payload,
            dump_result,
            verify,
            verify_data,
        })
    }

    pub(super) fn check_result(
        &self,
        task_id: usize,
        data: Vec<u8>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<()> {
        if self.dump_result {
            let hex_str = hex::encode(&data);
            if !proc_args.no_summary {
                println!("== Output of task {task_id}:");
            }
            println!("{hex_str}");
        }

        if !self.verify {
            return Ok(());
        }

        match self.action {
            KeylessAction::RsaSign(digest, padding) => self.verify_rsa(digest, padding, &data),
            KeylessAction::EcdsaSign(digest) => self.verify(digest, &data),
            KeylessAction::Ed25519Sign => self.verify_ed(&data),
            KeylessAction::Encrypt => {
                let decrypted = self.decrypt_data(&data)?;
                if decrypted != self.payload {
                    println!(" original: {data:?}\ndecrypted: {decrypted:?}");
                    Err(anyhow!("result data can't be decrypted to original"))
                } else {
                    Ok(())
                }
            }
            KeylessAction::RsaEncrypt(padding) => {
                let decrypted = self.decrypt_rsa_data(padding, &data)?;
                if decrypted != self.payload {
                    println!(" original: {data:?}\ndecrypted: {decrypted:?}");
                    Err(anyhow!("result data can't be decrypted to original"))
                } else {
                    Ok(())
                }
            }
            KeylessAction::RsaPrivateEncrypt(padding) => {
                let decrypted = self.rsa_public_decrypt_data(padding, &data)?;
                if decrypted != self.payload {
                    Err(anyhow!("result data can't be decrypted to original"))
                } else {
                    Ok(())
                }
            }
            _ => {
                if self.verify_data.is_empty() {
                    return Err(anyhow!("expected data is needed to verify the result"));
                }
                if self.verify_data != data {
                    Err(anyhow!(
                        "result data not match:\nexpected: {:?}\n   found: {:?}",
                        self.verify_data,
                        data
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    #[inline]
    pub(super) fn subject_key_id(&self) -> &[u8] {
        &self.public_key_ski
    }

    fn get_private_key(&self) -> anyhow::Result<&PKey<Private>> {
        self.private_key
            .as_ref()
            .ok_or_else(|| anyhow!("no private key set"))
    }

    pub(super) fn encrypt(&self) -> anyhow::Result<Vec<u8>> {
        let mut ctx = PkeyCtx::new(&self.public_key)
            .map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.encrypt_init()
            .map_err(|e| anyhow!("encrypt init failed: {e}"))?;

        let mut buf = Vec::new();
        ctx.encrypt_to_vec(&self.payload, &mut buf)
            .map_err(|e| anyhow!("encrypt failed: {e}"))?;
        Ok(buf)
    }

    pub(super) fn encrypt_rsa(&self, padding: KeylessRsaPadding) -> anyhow::Result<Vec<u8>> {
        let mut ctx = PkeyCtx::new(&self.public_key)
            .map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.encrypt_init()
            .map_err(|e| anyhow!("encrypt init failed: {e}"))?;
        ctx.set_rsa_padding(padding.into())
            .map_err(|e| anyhow!("failed to set rsa padding: {e}"))?;

        let mut buf = Vec::new();
        ctx.encrypt_to_vec(&self.payload, &mut buf)
            .map_err(|e| anyhow!("encrypt failed: {e}"))?;
        Ok(buf)
    }

    pub(super) fn decrypt(&self) -> anyhow::Result<Vec<u8>> {
        self.decrypt_data(&self.payload)
    }

    fn decrypt_data(&self, from: &[u8]) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let mut ctx =
            PkeyCtx::new(pkey).map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.decrypt_init()
            .map_err(|e| anyhow!("decrypt init failed: {e}"))?;

        let mut buf = Vec::new();
        ctx.decrypt_to_vec(from, &mut buf)
            .map_err(|e| anyhow!("decrypt failed: {e}"))?;
        Ok(buf)
    }

    pub(super) fn decrypt_rsa(&self, padding: KeylessRsaPadding) -> anyhow::Result<Vec<u8>> {
        self.decrypt_rsa_data(padding, &self.payload)
    }

    fn decrypt_rsa_data(&self, padding: KeylessRsaPadding, from: &[u8]) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let mut ctx =
            PkeyCtx::new(pkey).map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.decrypt_init()
            .map_err(|e| anyhow!("decrypt init failed: {e}"))?;
        ctx.set_rsa_padding(padding.into())
            .map_err(|e| anyhow!("failed to set rsa padding: {e}"))?;

        let mut buf = Vec::new();
        ctx.decrypt_to_vec(from, &mut buf)
            .map_err(|e| anyhow!("decrypt failed: {e}"))?;
        Ok(buf)
    }

    pub(super) fn sign(&self, digest: KeylessSignDigest) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let mut ctx =
            PkeyCtx::new(pkey).map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.sign_init()
            .map_err(|e| anyhow!("sign init failed: {e}"))?;
        ctx.set_signature_md(digest.md())
            .map_err(|e| anyhow!("failed to set signature digest type: {e}"))?;

        let mut buf = Vec::new();
        ctx.sign_to_vec(&self.payload, &mut buf)
            .map_err(|e| anyhow!("sign failed: {e}"))?;
        Ok(buf)
    }

    fn verify(&self, digest: KeylessSignDigest, sig: &[u8]) -> anyhow::Result<()> {
        let mut ctx = PkeyCtx::new(&self.public_key)
            .map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.verify_init()
            .map_err(|e| anyhow!("verify init failed: {e}"))?;
        ctx.set_signature_md(digest.md())
            .map_err(|e| anyhow!("failed to set signature digest type: {e}"))?;

        let verified = ctx
            .verify(&self.payload, sig)
            .map_err(|e| anyhow!("verify failed: {e}"))?;
        if verified {
            Ok(())
        } else {
            Err(anyhow!("sign verify data failed"))
        }
    }

    pub(super) fn sign_rsa(
        &self,
        digest: KeylessSignDigest,
        padding: KeylessRsaPadding,
    ) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let mut ctx =
            PkeyCtx::new(pkey).map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.sign_init()
            .map_err(|e| anyhow!("sign init failed: {e}"))?;
        ctx.set_signature_md(digest.md())
            .map_err(|e| anyhow!("failed to set signature digest type: {e}"))?;
        ctx.set_rsa_padding(padding.into())
            .map_err(|e| anyhow!("failed to set rsa padding type: {e}"))?;

        let mut buf = Vec::new();
        ctx.sign_to_vec(&self.payload, &mut buf)
            .map_err(|e| anyhow!("sign failed: {e}"))?;
        Ok(buf)
    }

    fn verify_rsa(
        &self,
        digest: KeylessSignDigest,
        padding: KeylessRsaPadding,
        sig: &[u8],
    ) -> anyhow::Result<()> {
        let mut ctx = PkeyCtx::new(&self.public_key)
            .map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.verify_init()
            .map_err(|e| anyhow!("verify init failed: {e}"))?;
        ctx.set_signature_md(digest.md())
            .map_err(|e| anyhow!("failed to set signature digest type: {e}"))?;
        ctx.set_rsa_padding(padding.into())
            .map_err(|e| anyhow!("failed to set rsa padding type: {e}"))?;

        let verified = ctx
            .verify(&self.payload, sig)
            .map_err(|e| anyhow!("verify failed: {e}"))?;
        if verified {
            Ok(())
        } else {
            Err(anyhow!("sign verify data failed"))
        }
    }

    pub(super) fn sign_ed(&self) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let mut ctx =
            PkeyCtx::new(pkey).map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.sign_init()
            .map_err(|e| anyhow!("sign init failed: {e}"))?;

        let mut buf = Vec::new();
        ctx.sign_to_vec(&self.payload, &mut buf)
            .map_err(|e| anyhow!("sign failed: {e}"))?;
        Ok(buf)
    }

    fn verify_ed(&self, sig: &[u8]) -> anyhow::Result<()> {
        let mut ctx = PkeyCtx::new(&self.public_key)
            .map_err(|e| anyhow!("failed to create EVP_PKEY_CTX: {e}"))?;
        ctx.verify_init()
            .map_err(|e| anyhow!("verify init failed: {e}"))?;

        let verified = ctx
            .verify(&self.payload, sig)
            .map_err(|e| anyhow!("verify failed: {e}"))?;
        if verified {
            Ok(())
        } else {
            Err(anyhow!("sign verify data failed"))
        }
    }

    pub(super) fn rsa_private_encrypt(
        &self,
        padding: KeylessRsaPadding,
    ) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let rsa = pkey
            .rsa()
            .map_err(|e| anyhow!("private key is not rsa: {e}"))?;

        let rsa_size = rsa.size() as usize;
        let mut output_buf = vec![0u8; rsa_size];

        let payload_len = self.payload.len();
        if payload_len > rsa_size {
            return Err(anyhow!(
                "payload length {payload_len} is larger than RSA size {rsa_size}"
            ));
        }

        let len = rsa
            .private_encrypt(&self.payload, &mut output_buf, padding.into())
            .map_err(|e| anyhow!("rsa private encrypt failed: {e}"))?;
        output_buf.truncate(len);
        Ok(output_buf)
    }

    pub(super) fn rsa_public_decrypt(&self, padding: KeylessRsaPadding) -> anyhow::Result<Vec<u8>> {
        self.rsa_public_decrypt_data(padding, &self.payload)
    }

    fn rsa_public_decrypt_data(
        &self,
        padding: KeylessRsaPadding,
        from: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let rsa = self
            .public_key
            .rsa()
            .map_err(|e| anyhow!("the cert is not a valid rsa cert: {e}"))?;

        let rsa_size = rsa.size() as usize;
        let mut output_buf = vec![0u8; rsa_size];

        let payload_len = from.len();
        if payload_len != rsa_size {
            return Err(anyhow!(
                "payload length {payload_len} is not equal to RSA size {rsa_size}"
            ));
        }

        let len = rsa
            .public_decrypt(from, &mut output_buf, padding.into())
            .map_err(|e| anyhow!("rsa public decrypt failed: {e}"))?;
        output_buf.truncate(len);
        Ok(output_buf)
    }
}

fn add_keyless_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new(ARG_CERT)
            .help("Target certificate file")
            .num_args(1)
            .long(ARG_CERT)
            .value_parser(value_parser!(PathBuf))
            .required_unless_present(ARG_PKEY)
            .value_hint(ValueHint::FilePath),
    )
    .arg(
        Arg::new(ARG_PKEY)
            .help("Target private key file")
            .num_args(1)
            .long(ARG_PKEY)
            .value_parser(value_parser!(PathBuf))
            .required_unless_present(ARG_CERT)
            .value_hint(ValueHint::FilePath),
    )
    .arg(
        Arg::new(ARG_SIGN)
            .help("Computes cryptographic signatures of data")
            .long(ARG_SIGN)
            .action(ArgAction::SetTrue)
            .requires(ARG_DIGEST_TYPE),
    )
    .arg(
        Arg::new(ARG_DECRYPT)
            .help("Decrypt data with the corresponding private key")
            .long(ARG_DECRYPT)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_ENCRYPT)
            .help("Encrypt data with the corresponding public key")
            .long(ARG_ENCRYPT)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_RSA_PRIVATE_ENCRYPT)
            .help("RSA Private Encrypt")
            .long(ARG_RSA_PRIVATE_ENCRYPT)
            .action(ArgAction::SetTrue)
            .requires(ARG_RSA_PADDING),
    )
    .arg(
        Arg::new(ARG_RSA_PUBLIC_DECRYPT)
            .help("RSA Public Decrypt")
            .long(ARG_RSA_PUBLIC_DECRYPT)
            .action(ArgAction::SetTrue)
            .requires(ARG_RSA_PADDING),
    )
    .group(
        ArgGroup::new("method")
            .args([
                ARG_SIGN,
                ARG_DECRYPT,
                ARG_ENCRYPT,
                ARG_RSA_PRIVATE_ENCRYPT,
                ARG_RSA_PUBLIC_DECRYPT,
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
    .arg(
        Arg::new(ARG_RSA_PADDING)
            .help("RSA Padding Type")
            .num_args(1)
            .long(ARG_RSA_PADDING)
            .value_parser(RSA_PADDING_VALUES)
            .default_value("PKCS1"),
    )
    .arg(
        Arg::new(ARG_PAYLOAD)
            .help("Payload data")
            .num_args(1)
            .required(true),
    )
    .arg(
        Arg::new(ARG_DUMP_RESULT)
            .help("Dump output use hex string")
            .action(ArgAction::SetTrue)
            .long(ARG_DUMP_RESULT),
    )
    .arg(
        Arg::new(ARG_VERIFY)
            .help("Verify the result")
            .action(ArgAction::SetTrue)
            .long(ARG_VERIFY),
    )
    .arg(
        Arg::new(ARG_VERIFY_DATA)
            .help("Verify the result with the same data")
            .num_args(1)
            .long(ARG_VERIFY_DATA),
    )
}

impl AppendKeylessArgs for Command {
    fn append_keyless_args(self) -> Self {
        add_keyless_args(self)
    }
}
