/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use libc::{c_int, c_uchar, c_uint};
use openssl_sys::RSA;

unsafe extern "C" {

    pub fn RSA_sign_ASN1_OCTET_STRING(
        dummy: c_int,
        m: *mut c_uchar,
        m_len: c_uint,
        sigret: *mut c_uchar,
        siglen: *mut c_uint,
        rsa: *mut RSA,
    ) -> c_int;
}
