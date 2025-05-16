/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use openssl::error::ErrorStack;
use openssl::x509::X509Extension;
use openssl::x509::extension::KeyUsage;

pub struct KeyUsageBuilder(KeyUsage);

impl KeyUsageBuilder {
    pub fn ca() -> Self {
        let mut usage = KeyUsage::new();
        usage.critical().key_cert_sign().crl_sign();
        KeyUsageBuilder(usage)
    }

    pub fn tls_general() -> Self {
        let mut usage = KeyUsage::new();
        usage
            .critical()
            .key_agreement()
            .digital_signature()
            .key_encipherment();
        KeyUsageBuilder(usage)
    }

    /// Edwards-curve Digital Signature Algorithm
    pub fn ed_dsa() -> Self {
        let mut usage = KeyUsage::new();
        usage.critical().digital_signature();
        KeyUsageBuilder(usage)
    }

    /// for CurveXXX for Diffie-Hellman
    pub fn x_dh() -> Self {
        let mut usage = KeyUsage::new();
        usage.critical().key_agreement();
        KeyUsageBuilder(usage)
    }

    pub fn tlcp_sign() -> Self {
        let mut usage = KeyUsage::new();
        usage.critical().non_repudiation().digital_signature();
        KeyUsageBuilder(usage)
    }

    pub fn tlcp_enc() -> Self {
        let mut usage = KeyUsage::new();
        usage
            .critical()
            .key_agreement()
            .key_encipherment()
            .data_encipherment();
        KeyUsageBuilder(usage)
    }

    pub fn build(&self) -> Result<X509Extension, ErrorStack> {
        self.0.build()
    }
}
