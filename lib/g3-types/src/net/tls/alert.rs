/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

pub enum TlsAlertType {
    Closure,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TlsAlert(i32);

macro_rules! def_const {
    ($name:ident, $value:literal) => {
        pub const $name: TlsAlert = TlsAlert($value);
    };
}

impl TlsAlert {
    // https://www.iana.org/assignments/tls-parameters/tls-parameters.xhtml#tls-parameters-6
    def_const!(CLOSE_NOTIFY, 0);
    def_const!(UNEXPECTED_MESSAGE, 10);
    def_const!(BAD_RECORD_MAC, 20);
    def_const!(DECRYPTION_FAILED, 21);
    def_const!(RECORD_OVERFLOW, 22);
    def_const!(DECOMPRESSION_FAILURE, 30);
    def_const!(HANDSHAKE_FAILURE, 40);
    def_const!(NO_CERTIFICATE, 41);
    def_const!(BAD_CERTIFICATE, 42);
    def_const!(UNSUPPORTED_CERTIFICATE, 43);
    def_const!(CERTIFICATE_REVOKED, 44);
    def_const!(CERTIFICATE_EXPIRED, 45);
    def_const!(CERTIFICATE_UNKNOWN, 46);
    def_const!(ILLEGAL_PARAMETER, 47);
    def_const!(UNKNOWN_CA, 48);
    def_const!(ACCESS_DENIED, 49);
    def_const!(DECODE_ERROR, 50);
    def_const!(DECRYPT_ERROR, 51);
    def_const!(TOO_MANY_CIDS_REQUESTED, 52);
    def_const!(EXPORT_RESTRICTION, 60);
    def_const!(PROTOCOL_VERSION, 70);
    def_const!(INSUFFICIENT_SECURITY, 71);
    def_const!(INTERNAL_ERROR, 80);
    def_const!(INAPPROPRIATE_FALLBACK, 86);
    def_const!(USER_CANCELED, 90);
    def_const!(NO_RENEGOTIATION, 100);
    def_const!(MISSING_EXTENSION, 109);
    def_const!(UNSUPPORTED_EXTENSION, 110);
    def_const!(CERTIFICATE_UNOBTAINABLE, 111);
    def_const!(UNRECOGNIZED_NAME, 112);
    def_const!(BAD_CERTIFICATE_STATUS_RESPONSE, 113);
    def_const!(BAD_CERTIFICATE_HASH_VALUE, 114);
    def_const!(UNKNOWN_PSK_IDENTITY, 115);
    def_const!(CERTIFICATE_REQUIRED, 116);
    def_const!(NO_APPLICATION_PROTOCOL, 120);
    def_const!(ECH_REQUIRED, 121);

    pub fn new(ret: i32) -> Self {
        TlsAlert(ret)
    }

    pub fn r#type(&self) -> TlsAlertType {
        if matches!(*self, Self::CLOSE_NOTIFY | Self::USER_CANCELED) {
            TlsAlertType::Closure
        } else {
            TlsAlertType::Error
        }
    }
}
