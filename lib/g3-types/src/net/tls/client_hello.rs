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

use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::{anyhow, Context};

pub enum ClientHelloExtAction {
    Keep(u16),
    Overwrite(u16, Vec<u8>),
    Add(u16, Vec<u8>),
}

impl FromStr for ClientHelloExtAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split(':');
        let id_s = iter
            .next()
            .ok_or_else(|| anyhow!("empty ext action string"))?;
        let id = u16::from_str(id_s).map_err(|e| anyhow!("invalid ext id {id_s}: {e}"))?;

        let Some(action_s) = iter.next() else {
            return Ok(ClientHelloExtAction::Keep(id));
        };

        let data_s = iter.next().unwrap_or_default();
        match action_s.to_lowercase().as_str() {
            "keep" => Ok(ClientHelloExtAction::Keep(id)),
            "overwrite" => {
                if data_s.is_empty() {
                    return Ok(ClientHelloExtAction::Overwrite(id, Vec::default()));
                }

                let data =
                    hex::decode(data_s).map_err(|e| anyhow!("invalid hex data {data_s}: {e}"))?;
                Ok(ClientHelloExtAction::Overwrite(id, data))
            }
            "add" => {
                if data_s.is_empty() {
                    return Ok(ClientHelloExtAction::Add(id, Vec::default()));
                }

                let data =
                    hex::decode(data_s).map_err(|e| anyhow!("invalid hex data {data_s}: {e}"))?;
                Ok(ClientHelloExtAction::Add(id, data))
            }
            _ => Err(anyhow!("invalid action {action_s}")),
        }
    }
}

#[derive(Default)]
pub struct ClientHelloRewriteRule {
    ext_actions: Vec<ClientHelloExtAction>,
}

impl ClientHelloRewriteRule {
    pub fn push_ext_action(&mut self, ext_action: ClientHelloExtAction) {
        self.ext_actions.push(ext_action);
    }

    pub fn push_ext_action_str(&mut self, s: &str) -> anyhow::Result<()> {
        for (i, action_s) in s.split(',').enumerate() {
            let action = ClientHelloExtAction::from_str(action_s)
                .context(format!("invalid ext action #{i}"))?;
            self.ext_actions.push(action);
        }
        Ok(())
    }

    pub fn rewrite(&self, p: &[u8]) -> Option<Vec<u8>> {
        let client_hello = ClientHelloMessage::parse(p)?;

        let mut output = Vec::with_capacity(p.len());

        output.extend_from_slice(&p[0..client_hello.ext_offset]);
        output.push(0); // should be set to ext length
        output.push(0); // should be set to ext length

        for ext in &self.ext_actions {
            match ext {
                ClientHelloExtAction::Keep(id) => {
                    if let Some((offset, end)) = client_hello.ext_map.get(id) {
                        output.extend_from_slice(&p[*offset..*end]);
                    }
                }
                ClientHelloExtAction::Overwrite(id, data) => {
                    if client_hello.ext_map.contains_key(id) {
                        write_ext(&mut output, *id, data);
                    }
                }
                ClientHelloExtAction::Add(id, data) => {
                    write_ext(&mut output, *id, data);
                }
            }
        }

        // rewrite extension length
        let ext_len = output.len() - client_hello.ext_offset - 2;
        output[client_hello.ext_offset] = ((ext_len >> 8) & 0xFF) as u8;
        output[client_hello.ext_offset + 1] = (ext_len & 0xFF) as u8;

        let record_len = output.len() - 5;
        output[3] = ((record_len >> 8) & 0xFF) as u8;
        output[4] = (record_len & 0xFF) as u8;

        let msg_len = record_len - 4;
        output[6] = 0;
        output[7] = ((msg_len >> 8) & 0xFF) as u8;
        output[8] = (msg_len & 0xFF) as u8;

        Some(output)
    }
}

fn write_ext(v: &mut Vec<u8>, id: u16, data: &[u8]) {
    let id_b = id.to_be_bytes();
    v.extend_from_slice(&id_b);
    let len = data.len() as u16;
    let len_b = len.to_be_bytes();
    v.extend_from_slice(&len_b);
    if len > 0 {
        v.extend_from_slice(data);
    }
}

struct ClientHelloMessage {
    ext_offset: usize,
    ext_map: BTreeMap<u16, (usize, usize)>,
}

impl ClientHelloMessage {
    fn parse(p: &[u8]) -> Option<Self> {
        debug_assert!(!p.is_empty());
        if p[0] != 0x16 {
            // check for Handshake
            return None;
        }
        let mut offset = 5usize; // skip record header
        debug_assert!(p.len() > 5);
        if p[offset] != 0x01 {
            // check for ClientHello
            return None;
        }
        offset += 4; // skip msg_type and length
        offset += 2; // skip version
        offset += 32; // skip tls1.3 random or tls1.2 timestamp + random
        debug_assert!(p.len() > offset);
        let session_id_len = p[offset] as usize;
        offset += 1 + session_id_len; // skip session id
        debug_assert!(p.len() > offset + 2);
        let cipher_suite_len = u16::from_be_bytes([p[offset], p[offset + 1]]) as usize;
        offset += 2 + cipher_suite_len;
        debug_assert!(p.len() > offset);
        let compress_methods = p[offset] as usize;
        offset += 1 + compress_methods;

        let mut client_hello = ClientHelloMessage {
            ext_offset: offset,
            ext_map: BTreeMap::new(),
        };
        client_hello.parse_ext_list(p);
        Some(client_hello)
    }

    fn parse_ext_list(&mut self, p: &[u8]) {
        let mut offset = self.ext_offset + 2;

        loop {
            let left = p.len() - offset;
            if left < 4 {
                return;
            }

            let ext_type = u16::from_be_bytes([p[offset], p[offset + 1]]);
            let ext_len = u16::from_be_bytes([p[offset + 2], p[offset + 3]]) as usize;
            let ext_end = offset + ext_len + 4;
            if p.len() < ext_end {
                return;
            }

            self.ext_map.insert(ext_type, (offset, ext_end));
            offset = ext_end;
        }
    }
}
