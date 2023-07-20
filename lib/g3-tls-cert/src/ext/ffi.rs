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

use libc::{c_char, c_int, c_uchar, c_uint};
use openssl_sys::{
    point_conversion_form_t, ASN1_OBJECT, BN_CTX, EC_GROUP, EC_POINT, EVP_MD, EVP_PKEY, RSA, X509,
    X509_ALGOR,
};

#[allow(non_camel_case_types)]
pub enum X509_PUBKEY {}

extern "C" {
    pub fn X509_pubkey_digest(
        data: *const X509,
        type_: *const EVP_MD,
        md: *mut c_uchar,
        len: *mut c_uint,
    ) -> c_int;
    pub fn X509_get_X509_PUBKEY(x: *const X509) -> *mut X509_PUBKEY;

    pub fn X509_PUBKEY_set(x: *mut *mut X509_PUBKEY, pkey: *mut EVP_PKEY) -> c_int;
    pub fn X509_PUBKEY_free(a: *mut X509_PUBKEY);
    pub fn X509_PUBKEY_get0_param(
        ppkalg: *mut *mut ASN1_OBJECT,
        pk: *mut *const c_uchar,
        ppklen: *mut c_int,
        pa: *mut *mut X509_ALGOR,
        pub_: *mut X509_PUBKEY,
    ) -> c_int;

    pub fn RSA_sign_ASN1_OCTET_STRING(
        dummy: c_int,
        m: *mut c_uchar,
        m_len: c_uint,
        sigret: *mut c_uchar,
        siglen: *mut c_uint,
        rsa: *mut RSA,
    ) -> c_int;

    pub fn EC_POINT_point2hex(
        group: *const EC_GROUP,
        p: *const EC_POINT,
        form: point_conversion_form_t,
        ctx: *mut BN_CTX,
    ) -> *mut c_char;
}
