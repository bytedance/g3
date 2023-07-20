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

use foreign_types::{ForeignType, ForeignTypeRef};
use openssl::bn::BigNumContextRef;
use openssl::ec::{EcGroupRef, EcPointRef};
use openssl::error::ErrorStack;
use openssl::string::OpensslString;
use openssl_sys::point_conversion_form_t::POINT_CONVERSION_COMPRESSED;

use super::ffi;

pub trait EcPointExt {
    fn to_hex_str_compressed(
        &self,
        group: &EcGroupRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<OpensslString, ErrorStack>;
}

impl EcPointExt for EcPointRef {
    fn to_hex_str_compressed(
        &self,
        group: &EcGroupRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<OpensslString, ErrorStack> {
        unsafe {
            let r = ffi::EC_POINT_point2hex(
                group.as_ptr(),
                self.as_ptr(),
                POINT_CONVERSION_COMPRESSED,
                ctx.as_ptr(),
            );
            if r.is_null() {
                return Err(ErrorStack::get());
            }
            Ok(OpensslString::from_ptr(r))
        }
    }
}
