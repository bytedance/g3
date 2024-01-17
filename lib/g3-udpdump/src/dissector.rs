/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use g3_dpi::Protocol;

const EXP_PDU_TAG_DISSECTOR_NAME: u8 = 12;
const EXP_PDU_TAG_DISSECTOR_TABLE_NAME: u8 = 14;
const EXP_PDU_TAG_DISSECTOR_TABLE_NAME_NUM_VAL: u8 = 32;

const EXP_PDU_TAG_DISSECTOR_TABLE_NUM_VAL_LEN: u8 = 4;

#[derive(Clone, Copy)]
pub enum ExportedPduDissectorHint {
    Protocol(Protocol),
    TcpPort(u16),
    TlsPort(u16),
}

impl ExportedPduDissectorHint {
    pub(crate) fn serialize(&self, buf: &mut Vec<u8>) {
        match self {
            ExportedPduDissectorHint::Protocol(protocol) => {
                let dissector = protocol.wireshark_dissector();
                if !dissector.is_empty() {
                    buf.extend_from_slice(&[0x00, EXP_PDU_TAG_DISSECTOR_NAME]);
                    let len = (dissector.len() & 0xFFFF) as u16;
                    buf.extend_from_slice(&len.to_be_bytes());
                    buf.extend_from_slice(dissector.as_bytes());
                }
            }
            ExportedPduDissectorHint::TcpPort(port) => {
                buf.extend_from_slice(&[
                    0x00,
                    EXP_PDU_TAG_DISSECTOR_TABLE_NAME,
                    0x00,
                    0x08,
                    b't',
                    b'c',
                    b'p',
                    b'.',
                    b'p',
                    b'o',
                    b'r',
                    b't',
                ]);
                serialize_dissector_table_name_num_val(buf, *port);
            }
            ExportedPduDissectorHint::TlsPort(port) => {
                buf.extend_from_slice(&[
                    0x00,
                    EXP_PDU_TAG_DISSECTOR_TABLE_NAME,
                    0x00,
                    0x08,
                    b't',
                    b'l',
                    b's',
                    b'.',
                    b'p',
                    b'o',
                    b'r',
                    b't',
                ]);
                serialize_dissector_table_name_num_val(buf, *port);
            }
        }
    }
}

fn serialize_dissector_table_name_num_val(buf: &mut Vec<u8>, port: u16) {
    let port = port.to_be_bytes();
    buf.extend_from_slice(&[
        0x00,
        EXP_PDU_TAG_DISSECTOR_TABLE_NAME_NUM_VAL,
        0x00,
        EXP_PDU_TAG_DISSECTOR_TABLE_NUM_VAL_LEN,
        0x00,
        0x00,
        port[0],
        port[1],
    ]);
}
