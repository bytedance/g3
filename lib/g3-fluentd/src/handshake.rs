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

use anyhow::anyhow;

use rmp::decode::Bytes;

pub(super) struct HeloMsgRef<'a> {
    pub(super) nonce: &'a [u8],
    pub(super) auth_salt: &'a [u8],
    keepalive: bool,
}

pub(super) struct PongMsgRef<'a> {
    pub(super) auth_result: bool,
    pub(super) reason: &'a str,
    pub(super) server_hostname: &'a str,
    pub(super) shared_key_digest: &'a str,
}

pub(super) fn parse_helo(buf: &[u8]) -> anyhow::Result<HeloMsgRef<'_>> {
    let mut bytes = Bytes::new(buf);

    let size = rmp::decode::read_array_len(&mut bytes)
        .map_err(|e| anyhow!("invalid helo msg array len: {e:?}"))?;
    if size != 2 {
        return Err(anyhow!("unexpected helo msg array len {size}, should be 2"));
    }

    let (msg_type, tail) = rmp::decode::read_str_from_slice(bytes.remaining_slice())
        .map_err(|e| anyhow!("invalid helo msg type: {e}"))?;
    if msg_type != "HELO" {
        return Err(anyhow!(
            "unexpected helo msg type {msg_type}, should be HELO"
        ));
    }
    bytes = Bytes::new(tail);

    let mut msg = HeloMsgRef {
        nonce: b"",
        auth_salt: b"",
        keepalive: true,
    };

    let map_len = rmp::decode::read_map_len(&mut bytes)
        .map_err(|e| anyhow!("invalid helo msg options len: {e:?}"))?;
    for i in 0..map_len {
        let (key, tail) = rmp::decode::read_str_from_slice(bytes.remaining_slice())
            .map_err(|e| anyhow!("invalid options #{i} key type: {e}"))?;
        bytes = Bytes::new(tail);
        match key {
            "nonce" => {
                if let Ok((value, tail)) = rmp::decode::read_str_from_slice(tail) {
                    msg.nonce = value.as_bytes();
                    bytes = Bytes::new(tail);
                } else if let Ok(len) = rmp::decode::read_bin_len(&mut bytes) {
                    let len = len as usize;
                    let remaining = bytes.remaining_slice();
                    if len > remaining.len() {
                        return Err(anyhow!(
                            "no enough space for the bin value of option '{key}'"
                        ));
                    } else if len > 0 {
                        msg.nonce = &bytes.remaining_slice()[0..len];
                        bytes = Bytes::new(&remaining[len..]);
                    }
                } else {
                    return Err(anyhow!(
                        "the value for '{key}' option should be valid 'str' or 'bin'"
                    ));
                };
            }
            "auth" => {
                if let Ok((value, tail)) = rmp::decode::read_str_from_slice(tail) {
                    msg.auth_salt = value.as_bytes();
                    bytes = Bytes::new(tail);
                } else if let Ok(len) = rmp::decode::read_bin_len(&mut bytes) {
                    let len = len as usize;
                    let remaining = bytes.remaining_slice();
                    if len > remaining.len() {
                        return Err(anyhow!("no enough space for the value of option '{key}'"));
                    } else {
                        msg.auth_salt = &bytes.remaining_slice()[0..len];
                        bytes = Bytes::new(&remaining[len..]);
                    }
                } else {
                    return Err(anyhow!(
                        "the value for '{key}' option should be valid 'str' or 'bin'"
                    ));
                };
            }
            "keepalive" => {
                let keepalive = rmp::decode::read_bool(&mut bytes)
                    .map_err(|e| anyhow!("invalid 'bool' value for {key} option: {e:?}"))?;
                msg.keepalive = keepalive;
            }
            _ => {}
        }
    }

    Ok(msg)
}

pub(super) fn parse_pong(buf: &[u8]) -> anyhow::Result<PongMsgRef<'_>> {
    let mut bytes = Bytes::new(buf);

    let size = rmp::decode::read_array_len(&mut bytes)
        .map_err(|e| anyhow!("invalid pong msg array len: {e:?}"))?;
    if size != 5 {
        return Err(anyhow!("unexpected pong msg array len {size}, should be 5"));
    }

    let (msg_type, tail) = rmp::decode::read_str_from_slice(bytes.remaining_slice())
        .map_err(|e| anyhow!("invalid pong msg type: {e}"))?;
    if msg_type != "PONG" {
        return Err(anyhow!(
            "unexpected pong msg type {msg_type}, should be PONG"
        ));
    }
    bytes = Bytes::new(tail);

    let auth_result = rmp::decode::read_bool(&mut bytes)
        .map_err(|e| anyhow!("invalid 'bool' value for 'auth_result': {e:?}"))?;

    let mut msg = PongMsgRef {
        auth_result,
        reason: "",
        server_hostname: "",
        shared_key_digest: "",
    };

    let (reason, tail) = rmp::decode::read_str_from_slice(bytes.remaining_slice())
        .map_err(|e| anyhow!("invalid 'str' value for 'reason': {e}"))?;
    msg.reason = reason;
    bytes = Bytes::new(tail);

    let (hostname, tail) = rmp::decode::read_str_from_slice(bytes.remaining_slice())
        .map_err(|e| anyhow!("invalid 'str' value for 'server_hostname': {e}"))?;
    msg.server_hostname = hostname;
    bytes = Bytes::new(tail);

    let (digest, _tail) = rmp::decode::read_str_from_slice(bytes.remaining_slice())
        .map_err(|e| anyhow!("invalid 'str' value for 'shared_key_hexdigest': {e}"))?;
    msg.shared_key_digest = digest;

    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helo1() {
        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x83, 0xa5, b'n', b'o', b'n', b'c', b'e', 0xc4,
            0x10, 0xc4, 0xbc, 0x83, 0x2c, 0x0c, 0xb8, 0x8e, 0xda, 0x3a, 0x03, 0xbf, 0x19, 0xab,
            0x51, 0xee, 0x80, 0xa4, b'a', b'u', b't', b'h', 0xc4, 0x10, 0x5d, 0x68, 0x91, 0x52,
            0x8a, 0xd3, 0xd1, 0x5d, 0x8a, 0xc7, 0xbc, 0x80, 0xe4, 0x0d, 0x30, 0x4c, 0xa9, b'k',
            b'e', b'e', b'p', b'a', b'l', b'i', b'v', b'e', 0xc3,
        ];

        let helo = parse_helo(buf).unwrap();
        assert!(helo.keepalive);
    }
}
