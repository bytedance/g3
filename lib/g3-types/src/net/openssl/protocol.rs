/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpensslProtocol {
    Ssl3,
    Tls1,
    Tls11,
    Tls12,
    Tls13,
    #[cfg(feature = "tongsuo")]
    Tlcp11,
}

impl FromStr for OpensslProtocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "ssl3" | "ssl30" | "ssl3.0" | "ssl3_0" => Ok(OpensslProtocol::Ssl3),
            "tls1" | "tls10" | "tls1.0" | "tls1_0" => Ok(OpensslProtocol::Tls1),
            "tls11" | "tls1.1" | "tls1_1" => Ok(OpensslProtocol::Tls11),
            "tls12" | "tls1.2" | "tls1_2" => Ok(OpensslProtocol::Tls12),
            "tls13" | "tls1.3" | "tls1_3" => Ok(OpensslProtocol::Tls13),
            #[cfg(feature = "tongsuo")]
            "tlcp" | "tlcp1.1" | "tlcp1_1" => Ok(OpensslProtocol::Tlcp11),
            _ => Err(anyhow!("unknown openssl protocol {s}")),
        }
    }
}
