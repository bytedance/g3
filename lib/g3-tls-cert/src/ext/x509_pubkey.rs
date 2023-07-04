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

use std::ptr;
use std::slice;

use foreign_types::{foreign_type, ForeignTypeRef};
use libc::{c_int, c_uchar};
use openssl::error::ErrorStack;
use openssl::pkey::{HasPublic, PKeyRef};

use super::ffi;

foreign_type! {
    ///
    type CType = ffi::X509_PUBKEY;
    fn drop = ffi::X509_PUBKEY_free;

    pub struct X509Pubkey;
    pub struct X509PubkeyRef;
}

impl X509Pubkey {
    pub fn from_pubkey<T>(key: &PKeyRef<T>) -> Result<Self, ErrorStack>
    where
        T: HasPublic,
    {
        let mut p = ptr::null_mut();
        let r = unsafe { ffi::X509_PUBKEY_set(&mut p as *mut _, key.as_ptr()) };
        if r <= 0 {
            Err(ErrorStack::get())
        } else {
            Ok(X509Pubkey(p))
        }
    }
}

impl X509PubkeyRef {
    pub fn encoded_bytes(&self) -> Result<&[u8], ErrorStack> {
        unsafe {
            let mut pk = ptr::null_mut() as *const c_uchar;
            let mut pkt_len: c_int = 0;
            let r = ffi::X509_PUBKEY_get0_param(
                ptr::null_mut(),
                &mut pk as *mut _,
                &mut pkt_len as *mut _,
                ptr::null_mut(),
                self.as_ptr(),
            );

            if r <= 0 {
                Err(ErrorStack::get())
            } else {
                Ok(slice::from_raw_parts(pk, pkt_len as usize))
            }
        }
    }
}
