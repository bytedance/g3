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

use std::str::FromStr;

use anyhow::anyhow;
use rmpv::ValueRef;

use g3_types::net::{TlsCertUsage, TlsServiceType};

pub fn as_tls_service_type(v: &ValueRef) -> anyhow::Result<TlsServiceType> {
    match v {
        ValueRef::String(s) => {
            if let Some(s) = s.as_str() {
                TlsServiceType::from_str(s)
                    .map_err(|_| anyhow!("invalid tls service type string {s}"))
            } else {
                Err(anyhow!("invalid utf-8 string"))
            }
        }
        ValueRef::Binary(b) => {
            let s =
                std::str::from_utf8(b).map_err(|e| anyhow!("invalid utf-8 string buffer: {e}"))?;
            TlsServiceType::from_str(s).map_err(|_| anyhow!("invalid tls service type string {s}"))
        }
        ValueRef::Integer(i) => {
            let u = i
                .as_u64()
                .ok_or_else(|| anyhow!("out of range integer value"))?;
            let u = u8::try_from(u).map_err(|e| anyhow!("invalid u8 value: {e}"))?;
            TlsServiceType::try_from(u).map_err(|_| anyhow!("invalid u8 tls server type value {u}"))
        }
        _ => Err(anyhow!(
            "msgpack value type for 'tls service type' should be 'binary' or 'string' or 'u8'"
        )),
    }
}

pub fn as_tls_cert_usage(v: &ValueRef) -> anyhow::Result<TlsCertUsage> {
    match v {
        ValueRef::String(s) => {
            if let Some(s) = s.as_str() {
                TlsCertUsage::from_str(s).map_err(|_| anyhow!("invalid tls cert usage string: {s}"))
            } else {
                Err(anyhow!("invalid utf-8 string"))
            }
        }
        ValueRef::Binary(b) => {
            let s =
                std::str::from_utf8(b).map_err(|e| anyhow!("invalid utf-8 string buffer: {e}"))?;
            TlsCertUsage::from_str(s).map_err(|_| anyhow!("invalid tls cert usage string: {s}"))
        }
        ValueRef::Integer(i) => {
            let u = i
                .as_u64()
                .ok_or_else(|| anyhow!("out of range integer value"))?;
            let u = u8::try_from(u).map_err(|e| anyhow!("invalid u8 value: {e}"))?;
            TlsCertUsage::try_from(u).map_err(|_| anyhow!("invalid u8 tls cert usage value {u}"))
        }
        _ => Err(anyhow!(
            "msgpack value type for 'tls service type' should be 'binary' or 'string' or 'u8'"
        )),
    }
}
