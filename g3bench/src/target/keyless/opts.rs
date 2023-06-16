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

use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use clap::{value_parser, Arg, ArgAction, ArgGroup, ArgMatches, Command, ValueHint};
use openssl::encrypt::{Decrypter, Encrypter};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{Id, PKey, Private, Public};
use openssl::rsa::Padding;
use openssl::sign::Signer;

use g3_tls_cert::ext::{X509Ext, X509Pubkey};

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
        let digest = MessageDigest::from(*self);
        if digest.size() != payload.len() {
            return Err(anyhow!(
                "payload size {} not match digest size {}",
                payload.len(),
                digest.size()
            ));
        }
        Ok(())
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

impl From<KeylessSignDigest> for MessageDigest {
    fn from(value: KeylessSignDigest) -> Self {
        match value {
            KeylessSignDigest::Md5Sha1 => MessageDigest::from_nid(Nid::MD5_SHA1).unwrap(),
            KeylessSignDigest::Sha1 => MessageDigest::sha1(),
            KeylessSignDigest::Sha224 => MessageDigest::sha224(),
            KeylessSignDigest::Sha256 => MessageDigest::sha256(),
            KeylessSignDigest::Sha384 => MessageDigest::sha384(),
            KeylessSignDigest::Sha512 => MessageDigest::sha512(),
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
}

impl KeylessGlobalArgs {
    pub(super) fn parse_args(args: &ArgMatches) -> anyhow::Result<Self> {
        let mut public_key_ski = None;

        let cert = if let Some(file) = args.get_one::<PathBuf>(ARG_CERT) {
            let cert = crate::target::openssl::load_certs(file)?.pop().unwrap();

            let ski = if let Some(o) = cert.subject_key_id() {
                o.as_slice().to_vec()
            } else {
                cert.pubkey_digest(MessageDigest::sha1())
                    .map_err(|e| anyhow!("failed to get sha1 hash of pubkey digest: {e}"))?
            };
            public_key_ski = Some(ski);

            Some(cert)
        } else {
            None
        };

        let private_key = if let Some(file) = args.get_one::<PathBuf>(ARG_PKEY) {
            let key = crate::target::openssl::load_key(file)?;

            // verify SKI match
            let x509_pubkey = X509Pubkey::from_pubkey(&key).map_err(|e| {
                anyhow!(
                    "failed to create x509 pubkey from key in file {}: {e}",
                    file.display()
                )
            })?;
            let encoded_bytes = x509_pubkey
                .encoded_bytes()
                .map_err(|e| anyhow!("failed to get encoded pubkey data: {e}"))?;
            let digest = openssl::hash::hash(MessageDigest::sha1(), encoded_bytes)
                .map_err(|e| anyhow!("failed to get sha1 hash of pubkey: {e}"))?;
            let ski = digest.to_vec();

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

        Ok(KeylessGlobalArgs {
            public_key,
            private_key,
            public_key_ski,
            action,
            payload,
            dump_result,
        })
    }

    pub(super) fn dump_result(&self, task_id: usize, data: Vec<u8>) {
        if self.dump_result {
            let hex_str = hex::encode(data);
            println!("== Output of task {task_id}:\n{hex_str}");
        }
    }

    #[inline]
    pub(super) fn subject_key_id(&self) -> &[u8] {
        &self.public_key_ski
    }

    #[inline]
    pub(super) fn public_key(&self) -> &PKey<Public> {
        &self.public_key
    }

    fn get_private_key(&self) -> anyhow::Result<&PKey<Private>> {
        self.private_key
            .as_ref()
            .ok_or_else(|| anyhow!("no private key set"))
    }

    fn get_encrypter(&self) -> anyhow::Result<Encrypter> {
        Encrypter::new(&self.public_key).map_err(|e| anyhow!("failed to create encrypter: {e}"))
    }

    pub(super) fn encrypt(&self) -> anyhow::Result<Vec<u8>> {
        let encrypter = self.get_encrypter()?;
        self.do_encrypt(encrypter)
    }

    pub(super) fn encrypt_rsa(&self, padding: KeylessRsaPadding) -> anyhow::Result<Vec<u8>> {
        let mut encrypter = self.get_encrypter()?;
        encrypter
            .set_rsa_padding(padding.into())
            .map_err(|e| anyhow!("failed to set rsa padding: {e}"))?;
        self.do_encrypt(encrypter)
    }

    fn do_encrypt(&self, encrypter: Encrypter) -> anyhow::Result<Vec<u8>> {
        let buffer_len = encrypter
            .encrypt_len(&self.payload)
            .map_err(|e| anyhow!("failed to get buffer length: {e}"))?;
        let mut encrypted = vec![0u8; buffer_len];
        let len = encrypter
            .encrypt(&self.payload, &mut encrypted)
            .map_err(|e| anyhow!("failed to encrypt data: {e}"))?;
        encrypted.truncate(len);
        Ok(encrypted)
    }

    fn get_decrypter(&self) -> anyhow::Result<Decrypter> {
        let pkey = self.get_private_key()?;
        Decrypter::new(pkey).map_err(|e| anyhow!("failed to create decrypter: {e}"))
    }

    pub(super) fn decrypt(&self) -> anyhow::Result<Vec<u8>> {
        let decrypter = self.get_decrypter()?;
        self.do_decrypt(decrypter)
    }

    pub(super) fn decrypt_rsa(&self, padding: KeylessRsaPadding) -> anyhow::Result<Vec<u8>> {
        let mut decrypter = self.get_decrypter()?;
        decrypter
            .set_rsa_padding(padding.into())
            .map_err(|e| anyhow!("failed to set rsa padding: {e}"))?;
        self.do_decrypt(decrypter)
    }

    fn do_decrypt(&self, decrypter: Decrypter) -> anyhow::Result<Vec<u8>> {
        let buffer_len = decrypter
            .decrypt_len(&self.payload)
            .map_err(|e| anyhow!("failed to get buffer length: {e}"))?;
        let mut decrypted = vec![0u8; buffer_len];
        let len = decrypter
            .decrypt(&self.payload, &mut decrypted)
            .map_err(|e| anyhow!("failed to decrypt data: {e}"))?;
        decrypted.truncate(len);
        Ok(decrypted)
    }

    pub(super) fn sign(&self, digest: KeylessSignDigest) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let signer = Signer::new(digest.into(), pkey)
            .map_err(|e| anyhow!("error when create signer: {e}"))?;
        self.do_sign(signer)
    }

    pub(super) fn sign_rsa(
        &self,
        digest: KeylessSignDigest,
        padding: KeylessRsaPadding,
    ) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let mut signer = Signer::new(digest.into(), pkey)
            .map_err(|e| anyhow!("error when create signer: {e}"))?;
        signer
            .set_rsa_padding(padding.into())
            .map_err(|e| anyhow!("failed to set rsa padding: {e}"))?;
        self.do_sign(signer)
    }

    pub(super) fn sign_ed(&self) -> anyhow::Result<Vec<u8>> {
        let pkey = self.get_private_key()?;
        let signer = Signer::new_without_digest(pkey)
            .map_err(|e| anyhow!("error when create signer: {e}"))?;
        self.do_sign(signer)
    }

    fn do_sign(&self, mut signer: Signer) -> anyhow::Result<Vec<u8>> {
        signer
            .update(&self.payload)
            .map_err(|e| anyhow!("failed to set payload data: {e}"))?;
        signer
            .sign_to_vec()
            .map_err(|e| anyhow!("sign failed: {e}"))
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
            .private_decrypt(&self.payload, &mut output_buf, padding.into())
            .map_err(|e| anyhow!("rsa private encrypt failed: {e}"))?;
        output_buf.truncate(len);
        Ok(output_buf)
    }

    pub(super) fn rsa_public_decrypt(&self, padding: KeylessRsaPadding) -> anyhow::Result<Vec<u8>> {
        let rsa = self
            .public_key
            .rsa()
            .map_err(|e| anyhow!("the cert is not a valid rsa cert: {e}"))?;

        let rsa_size = rsa.size() as usize;
        let mut output_buf = vec![0u8; rsa_size];

        let payload_len = self.payload.len();
        if payload_len != rsa_size {
            return Err(anyhow!(
                "payload length {payload_len} is not equal to RSA size {rsa_size}"
            ));
        }

        let len = rsa
            .public_decrypt(&self.payload, &mut output_buf, padding.into())
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
            .num_args(0)
            .long(ARG_SIGN)
            .action(ArgAction::SetTrue)
            .requires(ARG_DIGEST_TYPE),
    )
    .arg(
        Arg::new(ARG_DECRYPT)
            .help("Decrypt data with the corresponding private key")
            .num_args(0)
            .long(ARG_DECRYPT)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_ENCRYPT)
            .help("Encrypt data with the corresponding public key")
            .num_args(0)
            .long(ARG_ENCRYPT)
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_RSA_PRIVATE_ENCRYPT)
            .help("RSA Private Encrypt")
            .num_args(0)
            .long(ARG_RSA_PRIVATE_ENCRYPT)
            .action(ArgAction::SetTrue)
            .requires(ARG_RSA_PADDING),
    )
    .arg(
        Arg::new(ARG_RSA_PUBLIC_DECRYPT)
            .help("RSA Public Decrypt")
            .num_args(0)
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
            .num_args(0)
            .long(ARG_DUMP_RESULT),
    )
}

impl AppendKeylessArgs for Command {
    fn append_keyless_args(self) -> Self {
        add_keyless_args(self)
    }
}
