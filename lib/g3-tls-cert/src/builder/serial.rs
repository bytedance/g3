/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use openssl::asn1::Asn1Integer;
use openssl::bn::{BigNum, MsbOption};

pub fn random_16() -> anyhow::Result<Asn1Integer> {
    let mut bn = BigNum::new().map_err(|e| anyhow!("failed to create big num: {e}"))?;
    bn.rand(128, MsbOption::ONE, true)
        .map_err(|e| anyhow!("failed to generate random big num: {e}"))?;
    bn.to_asn1_integer()
        .map_err(|e| anyhow!("failed to convert bn to asn1 integer: {e}"))
}
