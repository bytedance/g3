/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use libc::{c_uchar, c_uint};
use openssl::error::ErrorStack;
use openssl::foreign_types::ForeignType;
use openssl::pkey::HasPrivate;
use openssl::rsa::Rsa;

use super::ffi;

pub trait RsaExt {
    fn sign_asn1_octet_string(&self, from: &[u8], to: &mut [u8]) -> Result<(), ErrorStack>;
}

impl<T: HasPrivate> RsaExt for Rsa<T> {
    fn sign_asn1_octet_string(&self, from: &[u8], to: &mut [u8]) -> Result<(), ErrorStack> {
        let mut len = to.len() as c_uint;
        unsafe {
            let r = ffi::RSA_sign_ASN1_OCTET_STRING(
                0,
                from.as_ptr().cast::<c_uchar>().cast_mut(),
                from.len() as c_uint,
                to.as_mut_ptr(),
                &mut len,
                self.as_ptr(),
            );
            if r != 1 {
                Err(ErrorStack::get())
            } else {
                Ok(())
            }
        }
    }
}
