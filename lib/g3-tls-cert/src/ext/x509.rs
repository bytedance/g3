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

use foreign_types::ForeignTypeRef;
use libc::c_uint;
use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::x509::X509Ref;

use super::{ffi, X509PubkeyRef};

pub trait X509Ext {
    fn pubkey(&self) -> Result<&X509PubkeyRef, ErrorStack>;
    fn pubkey_digest(&self, hash_type: MessageDigest) -> Result<Vec<u8>, ErrorStack>;
}

impl X509Ext for X509Ref {
    fn pubkey(&self) -> Result<&X509PubkeyRef, ErrorStack> {
        unsafe {
            let p = ffi::X509_get_X509_PUBKEY(self.as_ptr());
            if p.is_null() {
                Err(ErrorStack::get())
            } else {
                Ok(X509PubkeyRef::from_ptr(p))
            }
        }
    }

    fn pubkey_digest(&self, hash_type: MessageDigest) -> Result<Vec<u8>, ErrorStack> {
        unsafe {
            let mut buf = vec![0u8; hash_type.size()];
            let mut len = hash_type.size() as c_uint;
            let r = ffi::X509_pubkey_digest(
                self.as_ptr(),
                hash_type.as_ptr(),
                buf.as_mut_ptr() as *mut _,
                &mut len,
            );
            if r <= 0 {
                Err(ErrorStack::get())
            } else {
                Ok(buf)
            }
        }
    }
}
