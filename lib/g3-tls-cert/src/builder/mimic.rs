/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::anyhow;
use openssl::asn1::Asn1Time;
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{Id, PKey, Private};
use openssl::x509::extension::{AuthorityKeyIdentifier, KeyUsage, SubjectKeyIdentifier};
use openssl::x509::{X509Builder, X509Extension, X509ExtensionRef, X509Ref, X509};

use crate::ext::X509BuilderExt;

pub struct MimicCertBuilder<'a> {
    mimic_cert: &'a X509Ref,
    pkey: PKey<Private>,
    keep_serial: bool,
}

impl<'a> MimicCertBuilder<'a> {
    pub fn new(mimic_cert: &'a X509Ref) -> anyhow::Result<Self> {
        let pkey = mimic_cert
            .public_key()
            .map_err(|e| anyhow!("failed to get key for the mimic cert: {e}"))?;
        let pkey = match pkey.id() {
            Id::RSA => super::pkey::new_rsa(2048)?,
            Id::EC => super::pkey::new_ec256()?,
            #[cfg(not(feature = "no-sm2"))]
            Id::SM2 => super::pkey::new_sm2()?,
            Id::ED448 => super::pkey::new_ed448()?,
            Id::ED25519 => super::pkey::new_ed25519()?,
            Id::X448 => super::pkey::new_x448()?,
            Id::X25519 => super::pkey::new_x25519()?,
            id => return Err(anyhow!("unsupported pkey ID: {id:?}")),
        };
        Ok(MimicCertBuilder {
            mimic_cert,
            pkey,
            keep_serial: false,
        })
    }

    pub fn set_keep_serial(&mut self, keep: bool) {
        self.keep_serial = keep;
    }

    pub fn valid_seconds(&self) -> anyhow::Result<i32> {
        let not_after = self.mimic_cert.not_after();

        let t_now =
            Asn1Time::days_from_now(0).map_err(|e| anyhow!("failed to get now datatime: {e}"))?;
        let diff = t_now
            .diff(not_after)
            .map_err(|e| anyhow!("failed to get time diff: {e}"))?;
        if diff.days < 0 {
            Err(anyhow!("the mimic cert is already expired"))
        } else {
            Ok(diff.days * 3600 + diff.secs)
        }
    }

    #[inline]
    pub fn pkey(&self) -> &PKey<Private> {
        &self.pkey
    }

    fn build_with_usage(
        &self,
        ca_cert: &X509Ref,
        ca_key: &PKey<Private>,
        sign_digest: Option<MessageDigest>,
        key_usage: &X509ExtensionRef,
    ) -> anyhow::Result<X509> {
        let mut builder =
            X509Builder::new().map_err(|e| anyhow!("failed to create x509 builder {e}"))?;
        builder
            .set_pubkey(&self.pkey)
            .map_err(|e| anyhow!("failed to set pub key: {e}"))?;

        if self.keep_serial {
            builder
                .set_serial_number(self.mimic_cert.serial_number())
                .map_err(|e| anyhow!("failed to set serial number: {e}"))?;
        } else {
            let serial = super::serial::random_16()?;
            builder
                .set_serial_number(&serial)
                .map_err(|e| anyhow!("failed to set serial number: {e}"))?;
        }

        builder
            .set_not_before(self.mimic_cert.not_before())
            .map_err(|e| anyhow!("failed to set NotBefore: {e}"))?;
        builder
            .set_not_after(self.mimic_cert.not_after())
            .map_err(|e| anyhow!("failed to set NotAfter: {e}"))?;

        let cert_version = self.mimic_cert.version();
        builder
            .set_version(cert_version)
            .map_err(|e| anyhow!("failed to set x509 version 3: {e}"))?;

        builder
            .append_extension2(key_usage)
            .map_err(|e| anyhow!("failed to append KeyUsage extension: {e}"))?;

        let ext_key_usage_loc = self
            .mimic_cert
            .get_extension_location(Nid::EXT_KEY_USAGE, None)
            .ok_or_else(|| anyhow!("failed to get location of extended key usage extension"))?;
        let ext_key_usage = self
            .mimic_cert
            .get_extension(ext_key_usage_loc)
            .map_err(|e| {
                anyhow!(
                    "failed to get extended key usage extension at location {ext_key_usage_loc}: {e}"
                )
            })?;
        builder
            .append_extension2(ext_key_usage)
            .map_err(|e| anyhow!("failed to append ExtendedKeyUsage extension: {e}"))?;

        builder
            .set_subject_name(self.mimic_cert.subject_name())
            .map_err(|e| anyhow!("failed to set subject name: {e}"))?;
        if let Some(stack) = self.mimic_cert.subject_alt_names() {
            let san = X509Extension::new_subject_alt_name(stack, false)
                .map_err(|e| anyhow!("failed to create SubjectAlternativeName extension: {e}"))?;
            builder
                .append_extension(san)
                .map_err(|e| anyhow!("failed to append SubjectAlternativeName extension: {e}"))?;
        }

        if cert_version >= 2 {
            // X509v3
            let v3_ctx = builder.x509v3_context(Some(ca_cert), None);
            let ski_ext = if self
                .mimic_cert
                .get_extension_location(Nid::SUBJECT_KEY_IDENTIFIER, None)
                .is_some()
            {
                let ski = SubjectKeyIdentifier::new()
                    .build(&v3_ctx)
                    .map_err(|e| anyhow!("failed to build SubjectKeyIdentifier extension: {e} "))?;
                Some(ski)
            } else {
                None
            };
            let aki_ext = if self
                .mimic_cert
                .get_extension_location(Nid::AUTHORITY_KEY_IDENTIFIER, None)
                .is_some()
            {
                let mut aki_builder = AuthorityKeyIdentifier::new();
                aki_builder.keyid(false);
                let aki = aki_builder.build(&v3_ctx).map_err(|e| {
                    anyhow!("failed to build AuthorityKeyIdentifier extension: {e}")
                })?;
                Some(aki)
            } else {
                None
            };

            if let Some(ski) = ski_ext {
                builder
                    .append_extension(ski)
                    .map_err(|e| anyhow!("failed to append SubjectKeyIdentifier extension: {e}"))?;
            }
            if let Some(aki) = aki_ext {
                builder.append_extension(aki).map_err(|e| {
                    anyhow!("failed to append AuthorityKeyIdentifier extension: {e}")
                })?;
            }
        }

        builder
            .set_issuer_name(ca_cert.subject_name())
            .map_err(|e| anyhow!("failed to set issuer name: {e}"))?;
        builder
            .sign_with_optional_digest(ca_key, sign_digest)
            .map_err(|e| anyhow!("failed to sign: {e}"))?;

        Ok(builder.build())
    }

    pub fn build_tls_cert(
        &self,
        ca_cert: &X509Ref,
        ca_key: &PKey<Private>,
        sign_digest: Option<MessageDigest>,
    ) -> anyhow::Result<X509> {
        let key_usage_loc = self
            .mimic_cert
            .get_extension_location(Nid::KEY_USAGE, None)
            .ok_or_else(|| anyhow!("failed to get location of key usage extension"))?;
        let key_usage = self.mimic_cert.get_extension(key_usage_loc).map_err(|e| {
            anyhow!("failed to get key usage extension at location {key_usage_loc}: {e}")
        })?;

        self.build_with_usage(ca_cert, ca_key, sign_digest, key_usage)
    }

    pub fn build_tlcp_enc_cert(
        &self,
        ca_cert: &X509Ref,
        ca_key: &PKey<Private>,
        sign_digest: Option<MessageDigest>,
    ) -> anyhow::Result<X509> {
        let key_usage = KeyUsage::new()
            .critical()
            .key_agreement()
            .key_encipherment()
            .data_encipherment()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        self.build_with_usage(ca_cert, ca_key, sign_digest, &key_usage)
    }

    pub fn build_tlcp_sign_cert(
        &self,
        ca_cert: &X509Ref,
        ca_key: &PKey<Private>,
        sign_digest: Option<MessageDigest>,
    ) -> anyhow::Result<X509> {
        let key_usage = KeyUsage::new()
            .critical()
            .non_repudiation()
            .digital_signature()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        self.build_with_usage(ca_cert, ca_key, sign_digest, &key_usage)
    }
}
