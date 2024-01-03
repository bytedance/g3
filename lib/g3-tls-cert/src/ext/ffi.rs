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

use libc::{c_int, c_uchar, c_uint};
use openssl_sys::RSA;

extern "C" {

    pub fn RSA_sign_ASN1_OCTET_STRING(
        dummy: c_int,
        m: *mut c_uchar,
        m_len: c_uint,
        sigret: *mut c_uchar,
        siglen: *mut c_uint,
        rsa: *mut RSA,
    ) -> c_int;
}
