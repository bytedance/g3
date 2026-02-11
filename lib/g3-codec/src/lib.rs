/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

pub mod tlv;

#[cfg(feature = "leb128")]
pub mod leb128;

#[cfg(feature = "ber")]
pub mod ber;

#[cfg(feature = "thrift")]
pub mod thrift;

#[cfg(feature = "ldap")]
pub mod ldap;

#[cfg(feature = "quic")]
pub mod quic;
