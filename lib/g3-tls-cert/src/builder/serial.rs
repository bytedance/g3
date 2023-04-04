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
use openssl::asn1::Asn1Integer;
use openssl::bn::{BigNum, MsbOption};

pub fn random_16() -> anyhow::Result<Asn1Integer> {
    let mut bn = BigNum::new().map_err(|e| anyhow!("failed to create big num: {e}"))?;
    bn.rand(128, MsbOption::ONE, true)
        .map_err(|e| anyhow!("failed to generate random big num: {e}"))?;
    bn.to_asn1_integer()
        .map_err(|e| anyhow!("failed to convert bn to asn1 integer: {e}"))
}
