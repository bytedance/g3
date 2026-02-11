/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use anyhow::anyhow;
use tokio::io::AsyncRead;

use g3_codec::ldap::{LdapMessage, LdapMessageParseError};
use g3_io_ext::LimitedReadExt;

pub(crate) struct LdapMessageReceiver {
    max_message_size: usize,
    buffer: Box<[u8]>,
    received_len: usize,
    cur_message_len: usize,
}

impl LdapMessageReceiver {
    pub(crate) fn new(max_message_size: usize) -> Self {
        let buffer_size = max_message_size + 10;
        LdapMessageReceiver {
            max_message_size,
            buffer: vec![0; buffer_size].into_boxed_slice(),
            received_len: 0,
            cur_message_len: 0,
        }
    }

    fn consume_cur_message(&mut self) {
        if self.cur_message_len == 0 {
            return;
        }

        let left_size = self.received_len - self.cur_message_len;
        if left_size > 0 {
            self.buffer
                .copy_within(self.cur_message_len..self.received_len, 0);
        }
        self.received_len = left_size;
        self.cur_message_len = 0;
    }

    pub(crate) async fn recv<R>(&mut self, reader: &mut R) -> anyhow::Result<LdapMessage<'_>>
    where
        R: AsyncRead + Unpin,
    {
        self.consume_cur_message();

        // to workaround limitations of rust borrow checker
        let buffer_ptr = self.buffer.as_mut_ptr();
        let shadow_buffer = unsafe {
            std::ptr::slice_from_raw_parts_mut(buffer_ptr, self.buffer.len())
                .as_mut()
                .unwrap()
        };

        loop {
            if self.received_len > 0 {
                match LdapMessage::parse(&self.buffer[..self.received_len], self.max_message_size) {
                    Ok(message) => {
                        self.cur_message_len = message.encoded_size();
                        return Ok(message);
                    }
                    Err(LdapMessageParseError::NeedMoreData(_)) => {}
                    Err(e) => return Err(anyhow!("invalid ldap response message received: {e}")),
                }
            }

            let nr = reader
                .read_all_once(&mut shadow_buffer[self.received_len..])
                .await
                .map_err(|e| anyhow!("read io error: {e}"))?;
            if nr == 0 {
                return Err(anyhow!("ldap connection closed unexpected"));
            }
            self.received_len += nr;
        }
    }
}
