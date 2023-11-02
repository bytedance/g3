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
                from.as_ptr() as *mut c_uchar,
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
