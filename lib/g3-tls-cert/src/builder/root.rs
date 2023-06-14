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
use chrono::{Days, Utc};
use openssl::asn1::{Asn1Integer, Asn1Time};
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::{BasicConstraints, KeyUsage, SubjectKeyIdentifier};
use openssl::x509::{X509Builder, X509Extension, X509};

use super::{asn1_time_from_chrono, SubjectNameBuilder};
use crate::ext::X509BuilderExt;

pub struct RootCertBuilder {
    pkey: PKey<Private>,
    serial: Asn1Integer,
    key_usage: X509Extension,
    basic_constraints: X509Extension,
    not_before: Asn1Time,
    not_after: Asn1Time,
    subject_builder: SubjectNameBuilder,
}

macro_rules! impl_new {
    ($f:ident) => {
        pub fn $f() -> anyhow::Result<Self> {
            let pkey = super::pkey::$f()?;
            RootCertBuilder::with_pkey(pkey)
        }
    };
}

impl RootCertBuilder {
    impl_new!(new_ec224);
    impl_new!(new_ec256);
    impl_new!(new_ec384);
    impl_new!(new_ec521);
    impl_new!(new_sm2);
    impl_new!(new_ed25519);
    impl_new!(new_ed448);
    impl_new!(new_x25519);
    impl_new!(new_x448);

    pub fn new_rsa(bits: u32) -> anyhow::Result<Self> {
        let pkey = super::pkey::new_rsa(bits)?;
        RootCertBuilder::with_pkey(pkey)
    }

    fn with_pkey(pkey: PKey<Private>) -> anyhow::Result<Self> {
        let serial = super::serial::random_16()?;

        let key_usage = KeyUsage::new()
            .critical()
            .key_cert_sign()
            .build()
            .map_err(|e| anyhow!("failed to build KeyUsage extension: {e}"))?;

        let basic_constraints = BasicConstraints::new()
            .critical()
            .ca()
            .build()
            .map_err(|e| anyhow!("failed to build BasicConstraints extension: {e}"))?;

        let time_now = Utc::now();
        let time_before = time_now
            .checked_sub_days(Days::new(1))
            .ok_or(anyhow!("unable to get time before date"))?;
        let time_after = time_now
            .checked_add_days(Days::new(3650))
            .ok_or(anyhow!("unable to get time after date"))?;
        let not_before =
            asn1_time_from_chrono(&time_before).context("failed to get NotBefore time")?;
        let not_after =
            asn1_time_from_chrono(&time_after).context("failed to set NotAfter time")?;

        Ok(RootCertBuilder {
            pkey,
            serial,
            key_usage,
            basic_constraints,
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

    pub fn set_serial(&mut self, serial: Asn1Integer) {
        self.serial = serial;
    }

    pub fn build(&self, sign_digest: Option<MessageDigest>) -> anyhow::Result<X509> {
        let mut builder =
            X509Builder::new().map_err(|e| anyhow!("failed to create x509 builder {e}"))?;
        builder
            .set_pubkey(&self.pkey)
            .map_err(|e| anyhow!("failed to set pub key: {e}"))?;
        builder
            .set_serial_number(&self.serial)
            .map_err(|e| anyhow!("failed to set serial number: {e}"))?;

        builder
            .set_not_before(&self.not_before)
            .map_err(|e| anyhow!("failed to set NotBefore: {e}"))?;
        builder
            .set_not_after(&self.not_after)
            .map_err(|e| anyhow!("failed to set NotAfter: {e}"))?;

        builder
            .set_version(2)
            .map_err(|e| anyhow!("failed to set x509 version 3: {e}"))?;
        builder
            .append_extension2(&self.key_usage)
            .map_err(|e| anyhow!("failed to append KeyUsage extension: {e}"))?;
        builder
            .append_extension2(&self.basic_constraints)
            .map_err(|e| anyhow!("failed to append BasicConstraints extension: {e}"))?;

        let subject_name = self
            .subject_builder
            .build()
            .context("failed to build subject name")?;
        builder
            .set_subject_name(&subject_name)
            .map_err(|e| anyhow!("failed to set subject name: {e}"))?;

        let v3_ctx = builder.x509v3_context(None, None);
        let ski = SubjectKeyIdentifier::new()
            .build(&v3_ctx)
            .map_err(|e| anyhow!("failed to build SubjectKeyIdentifier extension: {e} "))?;
        builder
            .append_extension(ski)
            .map_err(|e| anyhow!("failed to append SubjectKeyIdentifier extension: {e}"))?;

        builder
            .set_issuer_name(&subject_name)
            .map_err(|e| anyhow!("failed to set issuer name: {e}"))?;
        builder
            .sign_with_optional_digest(&self.pkey, sign_digest)
            .map_err(|e| anyhow!("failed to sign: {e}"))?;

        Ok(builder.build())
    }
}
