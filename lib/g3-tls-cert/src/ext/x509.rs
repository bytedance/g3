/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::fmt;

use itertools::Itertools;
use openssl::pkey::Id;
use openssl::x509::X509;

pub trait X509Ext {
    fn dump(&self) -> impl fmt::Display;
}

impl X509Ext for X509 {
    fn dump(&self) -> impl fmt::Display {
        DumpX509Cert { crt: self }
    }
}

struct DumpX509Cert<'a> {
    crt: &'a X509,
}

impl fmt::Display for DumpX509Cert<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let serial = if let Ok(bn) = self.crt.serial_number().to_bn()
            && let Ok(serial) = bn.to_dec_str()
        {
            Some(serial)
        } else {
            None
        };
        let serial: &str = serial.as_ref().map(|s| s.as_ref()).unwrap_or_default();

        let (pubkey_type, pubkey_bits) = match self.crt.public_key() {
            Ok(pkey) => (pkey.id(), pkey.bits()),
            Err(_) => (Id::from_raw(0), 0),
        };

        let san = self
            .crt
            .subject_alt_names()
            .map(|san_stack| san_stack.iter().map(|gn| format!("{:?}", gn)).join(", "))
            .unwrap_or_default();

        let key_usage = self.crt.key_usage().unwrap_or_default();

        f.write_fmt(format_args!(
            "X509 Certificate:\n\
                 ├─ Serial: {}\n\
                 ├─ Subject: {:?}\n\
                 ├─ Issuer: {:?}\n\
                 ├─ Version: {}\n\
                 ├─ Validity:\n\
                 │   ├─ NotBefore: {}\n\
                 │   └─ NotAfter:  {}\n\
                 ├─ Signature Algorithm: {}\n\
                 ├─ Public Key: {:?} ({} bits)\n\
                 ├─ Key Usage: {}\n\
                 └─ Subject Alt Names: {}\n",
            serial,
            self.crt.subject_name(),
            self.crt.issuer_name(),
            self.crt.version() + 1,
            self.crt.not_before(),
            self.crt.not_after(),
            self.crt.signature_algorithm().object(),
            pubkey_type,
            pubkey_bits,
            key_usage,
            san
        ))
    }
}
