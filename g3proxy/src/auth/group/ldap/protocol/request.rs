/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use g3_codec::ber::BerLengthEncoder;

const MAX_MESSAGE_ID: u8 = 0x7F;
const MIN_MESSAGE_ID: u8 = 1;

pub(crate) struct SimpleBindRequestEncoder {
    message_id: u8,
    bind_dn_length_encoder: BerLengthEncoder,
    password_length_encoder: BerLengthEncoder,
    request_length_encoder: BerLengthEncoder,
    message_length_encoder: BerLengthEncoder,
    request_buf: Vec<u8>,
}

impl Default for SimpleBindRequestEncoder {
    fn default() -> Self {
        SimpleBindRequestEncoder {
            message_id: MAX_MESSAGE_ID,
            bind_dn_length_encoder: Default::default(),
            password_length_encoder: Default::default(),
            request_length_encoder: Default::default(),
            message_length_encoder: Default::default(),
            request_buf: Vec::with_capacity(256),
        }
    }
}

impl SimpleBindRequestEncoder {
    pub(crate) fn reset(&mut self) {
        self.message_id = MAX_MESSAGE_ID;
    }

    pub(crate) fn message_id(&self) -> u32 {
        self.message_id as u32
    }

    pub(crate) fn encode(&mut self, bind_dn: &str, password: &str) -> &[u8] {
        self.message_id += 1;
        if self.message_id > MAX_MESSAGE_ID {
            self.message_id = MIN_MESSAGE_ID;
        }

        let bind_dn_len = bind_dn.len();
        let bind_dn_length_bytes = self.bind_dn_length_encoder.encode(bind_dn_len);
        let bind_dn_encoded_len = 1 + bind_dn_length_bytes.len() + bind_dn_len;

        let password_len = password.len();
        let password_length_bytes = self.password_length_encoder.encode(password_len);
        let password_encoded_len = 1 + password_length_bytes.len() + password_len;

        let request_len = 3 + bind_dn_encoded_len + password_encoded_len;
        let request_length_bytes = self.request_length_encoder.encode(request_len);
        let request_encoded_len = 1 + request_length_bytes.len() + request_len;

        let message_len = 3 + request_encoded_len;
        let message_length_bytes = self.message_length_encoder.encode(message_len);
        let message_encoded_len = 1 + message_length_bytes.len() + message_len;

        self.request_buf.clear();
        self.request_buf.reserve(message_encoded_len);

        // Begin the LDAPMessage sequence
        self.request_buf.push(0x30);
        self.request_buf.extend_from_slice(message_length_bytes);

        // The message ID
        self.request_buf.push(0x02);
        self.request_buf.push(0x01);
        self.request_buf.push(self.message_id); // the message is always <= 0x7F

        // Begin the bind request protocol op
        self.request_buf.push(0x60);
        self.request_buf.extend_from_slice(request_length_bytes);

        // The LDAP protocol version (integer value 3)
        self.request_buf.extend_from_slice(&[0x02, 0x01, 0x03]);

        // The bind DN
        self.request_buf.push(0x04);
        self.request_buf.extend_from_slice(bind_dn_length_bytes);
        self.request_buf.extend_from_slice(bind_dn.as_bytes());

        // The password
        self.request_buf.push(0x80);
        self.request_buf.extend_from_slice(password_length_bytes);
        self.request_buf.extend_from_slice(password.as_bytes());

        &self.request_buf
    }

    pub(crate) fn unbind_sequence(&mut self) -> [u8; 7] {
        self.message_id += 1;
        if self.message_id > MAX_MESSAGE_ID {
            self.message_id = MIN_MESSAGE_ID;
        }

        [0x30, 0x05, 0x02, 0x01, self.message_id, 0x42, 0x00]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        let mut encoder = SimpleBindRequestEncoder::default();
        let bind_dn = "uid=jdoe,ou=People,dc=example,dc=com";

        let request = encoder.encode(bind_dn, "secret123");
        assert_eq!(
            request,
            [
                0x30, 0x39, // Begin the LDAPMessage sequence
                0x02, 0x01, 0x01, // The message ID (integer value 1)
                0x60, 0x34, // Begin the bind request protocol op
                0x02, 0x01, 0x03, // The LDAP protocol version (integer value 3)
                0x04, 0x24, b'u', b'i', b'd', b'=', b'j', b'd', b'o', b'e', b',', b'o', b'u', b'=',
                b'P', b'e', b'o', b'p', b'l', b'e', b',', b'd', b'c', b'=', b'e', b'x', b'a', b'm',
                b'p', b'l', b'e', b',', b'd', b'c', b'=', b'c', b'o', b'm', // base dn
                0x80, 0x09, b's', b'e', b'c', b'r', b'e', b't', b'1', b'2', b'3', // password
            ]
        );
    }
}
