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

use anyhow::anyhow;
use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;

pub fn new_ec224() -> anyhow::Result<PKey<Private>> {
    let group = EcGroup::from_curve_name(Nid::SECP224R1)
        .map_err(|e| anyhow!("failed to get P-224 ec group: {e}"))?;
    new_ec(&group)
}

pub fn new_ec256() -> anyhow::Result<PKey<Private>> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)
        .map_err(|e| anyhow!("failed to get P-256 ec group: {e}"))?;
    new_ec(&group)
}

pub fn new_ec384() -> anyhow::Result<PKey<Private>> {
    let group = EcGroup::from_curve_name(Nid::SECP384R1)
        .map_err(|e| anyhow!("failed to get P-384 ec group: {e}"))?;
    new_ec(&group)
}

pub fn new_ec521() -> anyhow::Result<PKey<Private>> {
    let group = EcGroup::from_curve_name(Nid::SECP521R1)
        .map_err(|e| anyhow!("failed to get P-521 ec group: {e}"))?;
    new_ec(&group)
}

pub fn new_sm2() -> anyhow::Result<PKey<Private>> {
    // TODO use Nid::SM2 after supported
    let group = EcGroup::from_curve_name(Nid::from_raw(1172))
        .map_err(|e| anyhow!("failed to get SM2 ec group: {e}"))?;
    new_ec(&group)
}

fn new_ec(group: &EcGroup) -> anyhow::Result<PKey<Private>> {
    let ec_key = EcKey::generate(group).map_err(|e| anyhow!("failed to generate ec key: {e}"))?;
    PKey::from_ec_key(ec_key).map_err(|e| anyhow!("failed to convert ec key to pkey: {e}"))
}

pub fn new_ed25519() -> anyhow::Result<PKey<Private>> {
    PKey::generate_ed25519().map_err(|e| anyhow!("failed to generate ed25519 pkey: {e}"))
}

pub fn new_ed448() -> anyhow::Result<PKey<Private>> {
    PKey::generate_ed448().map_err(|e| anyhow!("failed to generate ed448 pkey: {e}"))
}

pub fn new_x25519() -> anyhow::Result<PKey<Private>> {
    PKey::generate_x25519().map_err(|e| anyhow!("failed to generate x25519 pkey: {e}"))
}

pub fn new_x448() -> anyhow::Result<PKey<Private>> {
    PKey::generate_x448().map_err(|e| anyhow!("failed to generate x448 pkey: {e}"))
}

pub fn new_rsa(bits: u32) -> anyhow::Result<PKey<Private>> {
    let rsa_key =
        Rsa::generate(bits).map_err(|e| anyhow!("failed to generate rsa {bits} keypair: {e}"))?;
    PKey::from_rsa(rsa_key).map_err(|e| anyhow!("failed to convert rsa key to pkey: {e}"))
}
