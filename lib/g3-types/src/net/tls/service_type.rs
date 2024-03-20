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

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum TlsServiceType {
    Http,
    Smtp,
}

impl TlsServiceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TlsServiceType::Http => "http",
            TlsServiceType::Smtp => "smtp",
        }
    }
}

impl fmt::Display for TlsServiceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub struct InvalidServiceType;

impl fmt::Display for InvalidServiceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unsupported tls service type")
    }
}

impl FromStr for TlsServiceType {
    type Err = InvalidServiceType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" | "HTTP" => Ok(TlsServiceType::Http),
            "smtp" | "SMTP" => Ok(TlsServiceType::Smtp),
            _ => Err(InvalidServiceType),
        }
    }
}
