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

use std::str::FromStr;

use anyhow::anyhow;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpensslProtocol {
    Ssl3,
    Tls1,
    Tls11,
    Tls12,
    Tls13,
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
            _ => Err(anyhow!("")),
        }
    }
}
