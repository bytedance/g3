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

use anyhow::{Context, anyhow};
use chrono::{Days, Utc};
use openssl::asn1::{Asn1Integer, Asn1Time};
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::{
    AuthorityKeyIdentifier, ExtendedKeyUsage, SubjectAlternativeName, SubjectKeyIdentifier,
};
use openssl::x509::{X509, X509Builder, X509Extension, X509Name, X509Ref};

use g3_types::net::Host;

use super::{KeyUsageBuilder, SubjectNameBuilder, asn1_time_from_chrono};
use crate::ext::X509BuilderExt;

pub struct ClientCertBuilder {
    pkey: PKey<Private>,
    serial: Asn1Integer,
    key_usage: X509Extension,
    ext_key_usage: X509Extension,
    not_before: Asn1Time,
    not_after: Asn1Time,
    subject_builder: SubjectNameBuilder,
}

pub struct TlsClientCertBuilder {}

#[cfg(osslconf = "OPENSSL_NO_SM2")]
macro_rules! impl_no {
    ($f:ident, $a:literal) => {
        pub fn $f() -> anyhow::Result<ClientCertBuilder> {
            Err(anyhow!("{} is not supported", $a))
        }
    };
}

macro_rules! tls_impl_new {
    ($f:ident) => {
        pub fn $f() -> anyhow::Result<ClientCertBuilder> {
            let pkey = super::pkey::$f()?;
            TlsClientCertBuilder::with_pkey(pkey)
        }
    };
}

impl TlsClientCertBuilder {
    tls_impl_new!(new_ec224);
    tls_impl_new!(new_ec256);
    tls_impl_new!(new_ec384);
    tls_impl_new!(new_ec521);

    #[cfg(not(osslconf = "OPENSSL_NO_SM2"))]
    tls_impl_new!(new_sm2);
    #[cfg(osslconf = "OPENSSL_NO_SM2")]
    impl_no!(new_sm2, "SM2");

    pub fn new_ed25519() -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_ed25519()?;
        let key_usage = KeyUsageBuilder::ed_dsa()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        ClientCertBuilder::new(pkey, key_usage)
    }

    pub fn new_ed448() -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_ed448()?;
        let key_usage = KeyUsageBuilder::ed_dsa()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        ClientCertBuilder::new(pkey, key_usage)
    }

    pub fn new_x25519() -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_x25519()?;
        let key_usage = KeyUsageBuilder::x_dh()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        ClientCertBuilder::new(pkey, key_usage)
    }

    pub fn new_x448() -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_x448()?;
        let key_usage = KeyUsageBuilder::x_dh()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        ClientCertBuilder::new(pkey, key_usage)
    }

    pub fn new_rsa(bits: u32) -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_rsa(bits)?;
        TlsClientCertBuilder::with_pkey(pkey)
    }

    fn with_pkey(pkey: PKey<Private>) -> anyhow::Result<ClientCertBuilder> {
        let key_usage = KeyUsageBuilder::tls_general()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        ClientCertBuilder::new(pkey, key_usage)
    }
}

pub struct TlcpClientSignCertBuilder {}

impl TlcpClientSignCertBuilder {
    #[cfg(not(osslconf = "OPENSSL_NO_SM2"))]
    pub fn new_sm2() -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_sm2()?;
        TlcpClientSignCertBuilder::with_pkey(pkey)
    }
    #[cfg(osslconf = "OPENSSL_NO_SM2")]
    impl_no!(new_sm2, "SM2");

    pub fn new_rsa(bits: u32) -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_rsa(bits)?;
        TlcpClientSignCertBuilder::with_pkey(pkey)
    }

    fn with_pkey(pkey: PKey<Private>) -> anyhow::Result<ClientCertBuilder> {
        let key_usage = KeyUsageBuilder::tlcp_sign()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        ClientCertBuilder::new(pkey, key_usage)
    }
}

pub struct TlcpClientEncCertBuilder {}

impl TlcpClientEncCertBuilder {
    #[cfg(not(osslconf = "OPENSSL_NO_SM2"))]
    pub fn new_sm2() -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_sm2()?;
        TlcpClientEncCertBuilder::with_pkey(pkey)
    }
    #[cfg(osslconf = "OPENSSL_NO_SM2")]
    impl_no!(new_sm2, "SM2");

    pub fn new_rsa(bits: u32) -> anyhow::Result<ClientCertBuilder> {
        let pkey = super::pkey::new_rsa(bits)?;
        TlcpClientEncCertBuilder::with_pkey(pkey)
    }

    fn with_pkey(pkey: PKey<Private>) -> anyhow::Result<ClientCertBuilder> {
        let key_usage = KeyUsageBuilder::tlcp_enc()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;
        ClientCertBuilder::new(pkey, key_usage)
    }
}

impl ClientCertBuilder {
    pub fn new(pkey: PKey<Private>, key_usage: X509Extension) -> anyhow::Result<Self> {
        let serial = super::serial::random_16()?;

        let ext_key_usage = ExtendedKeyUsage::new()
            .client_auth()
            .build()
            .map_err(|e| anyhow!("failed to build ExtendedKeyUsage extension: {e}"))?;

        let time_now = Utc::now();
        let time_before = time_now
            .checked_sub_days(Days::new(1))
            .ok_or(anyhow!("unable to get time before date"))?;
        let time_after = time_now
            .checked_add_days(Days::new(365))
            .ok_or(anyhow!("unable to get time after date"))?;
        let not_before =
            asn1_time_from_chrono(&time_before).context("failed to get NotBefore time")?;
        let not_after =
            asn1_time_from_chrono(&time_after).context("failed to set NotAfter time")?;

        Ok(ClientCertBuilder {
            pkey,
            serial,
            key_usage,
            ext_key_usage,
            not_before,
            not_after,
            subject_builder: SubjectNameBuilder::default(),
        })
    }

    #[inline]
    pub fn subject_builder_mut(&mut self) -> &mut SubjectNameBuilder {
        &mut self.subject_builder
    }

    #[inline]
    pub fn subject_builder(&self) -> &SubjectNameBuilder {
        &self.subject_builder
    }

    #[inline]
    pub fn pkey(&self) -> &PKey<Private> {
        &self.pkey
    }

    pub fn set_pkey(&mut self, pkey: PKey<Private>) {
        self.pkey = pkey;
    }

    pub fn set_serial(&mut self, serial: Asn1Integer) {
        self.serial = serial;
    }

    pub fn build_for_host(
        &self,
        host: &Host,
        ca_cert: &X509Ref,
        ca_key: &PKey<Private>,
        sign_digest: Option<MessageDigest>,
    ) -> anyhow::Result<X509> {
        let mut san = SubjectAlternativeName::new();
        let subject_name = match host {
            Host::Domain(domain) => {
                san.dns(domain);
                self.subject_builder
                    .build_with_default_common_name(domain)
                    .context("failed to build subject name")?
            }
            Host::Ip(ip) => {
                let text = ip.to_string();
                san.ip(&text);
                self.subject_builder
                    .build_with_default_common_name(&text)
                    .context("failed to build subject name")?
            }
        };
        self.build_with_subject(&subject_name, san, ca_cert, ca_key, sign_digest)
    }

    pub fn build_with_subject(
        &self,
        subject_name: &X509Name,
        subject_alt_name: SubjectAlternativeName,
        ca_cert: &X509Ref,
        ca_key: &PKey<Private>,
        sign_digest: Option<MessageDigest>,
    ) -> anyhow::Result<X509> {
        let mut builder =
            X509Builder::new().map_err(|e| anyhow!("failed to create x509 builder {e}"))?;
        builder
            .set_pubkey(&self.pkey)
            .map_err(|e| anyhow!("failed to set pub key: {e}"))?;
        builder
            .set_serial_number(&self.serial)
            .map_err(|e| anyhow!("failed to set serial number: {e}"))?;

        let not_before = if ca_cert.not_before() > self.not_after {
            ca_cert.not_before()
        } else {
            &self.not_before
        };
        builder
            .set_not_before(not_before)
            .map_err(|e| anyhow!("failed to set NotBefore: {e}"))?;
        let not_after = if ca_cert.not_after() < self.not_after {
            ca_cert.not_after()
        } else {
            &self.not_after
        };
        builder
            .set_not_after(not_after)
            .map_err(|e| anyhow!("failed to set NotAfter: {e}"))?;

        builder
            .set_version(2)
            .map_err(|e| anyhow!("failed to set x509 version 3: {e}"))?;
        builder
            .append_extension2(&self.key_usage)
            .map_err(|e| anyhow!("failed to append KeyUsage extension: {e}"))?;
        builder
            .append_extension2(&self.ext_key_usage)
            .map_err(|e| anyhow!("failed to append ExtendedKeyUsage extension: {e}"))?;

        builder
            .set_subject_name(subject_name)
            .map_err(|e| anyhow!("failed to set subject name: {e}"))?;

        let v3_ctx = builder.x509v3_context(Some(ca_cert), None);
        let san = subject_alt_name
            .build(&v3_ctx)
            .map_err(|e| anyhow!("failed to build SubjectAlternativeName extension: {e}"))?;
        let ski = SubjectKeyIdentifier::new()
            .build(&v3_ctx)
            .map_err(|e| anyhow!("failed to build SubjectKeyIdentifier extension: {e} "))?;
        let mut aki_builder = AuthorityKeyIdentifier::new();
        aki_builder.keyid(false);
        let aki = aki_builder
            .build(&v3_ctx)
            .map_err(|e| anyhow!("failed to build AuthorityKeyIdentifier extension: {e}"))?;

        builder
            .append_extension(san)
            .map_err(|e| anyhow!("failed to append SubjectAlternativeName extension: {e}"))?;
        builder
            .append_extension(ski)
            .map_err(|e| anyhow!("failed to append SubjectKeyIdentifier extension: {e}"))?;
        builder
            .append_extension(aki)
            .map_err(|e| anyhow!("failed to append AuthorityKeyIdentifier extension: {e}"))?;

        builder
            .set_issuer_name(ca_cert.subject_name())
            .map_err(|e| anyhow!("failed to set issuer name: {e}"))?;
        builder
            .sign_with_optional_digest(ca_key, sign_digest)
            .map_err(|e| anyhow!("failed to sign: {e}"))?;

        Ok(builder.build())
    }
}
