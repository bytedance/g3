/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use rmp::decode::Bytes;

#[derive(Debug)]
pub(super) struct HeloMsgRef<'a> {
    pub(super) nonce: &'a [u8],
    pub(super) auth_salt: &'a [u8],
    keepalive: bool,
}

#[derive(Debug)]
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
    fn parse_helo_ok() {
        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x83, 0xa5, b'n', b'o', b'n', b'c', b'e', 0xc4,
            0x10, 0xc4, 0xbc, 0x83, 0x2c, 0x0c, 0xb8, 0x8e, 0xda, 0x3a, 0x03, 0xbf, 0x19, 0xab,
            0x51, 0xee, 0x80, 0xa4, b'a', b'u', b't', b'h', 0xc4, 0x10, 0x5d, 0x68, 0x91, 0x52,
            0x8a, 0xd3, 0xd1, 0x5d, 0x8a, 0xc7, 0xbc, 0x80, 0xe4, 0x0d, 0x30, 0x4c, 0xa9, b'k',
            b'e', b'e', b'p', b'a', b'l', b'i', b'v', b'e', 0xc3,
        ];
        let helo = parse_helo(buf).unwrap();
        assert_eq!(
            helo.nonce,
            &[
                0xc4, 0xbc, 0x83, 0x2c, 0x0c, 0xb8, 0x8e, 0xda, 0x3a, 0x03, 0xbf, 0x19, 0xab, 0x51,
                0xee, 0x80,
            ]
        );
        assert_eq!(
            helo.auth_salt,
            &[
                0x5d, 0x68, 0x91, 0x52, 0x8a, 0xd3, 0xd1, 0x5d, 0x8a, 0xc7, 0xbc, 0x80, 0xe4, 0x0d,
                0x30, 0x4c,
            ]
        );
        assert!(helo.keepalive);

        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x83, 0xa5, b'n', b'o', b'n', b'c', b'e', 0xa6,
            b'n', b'o', b'n', b'c', b'e', b'1', 0xa4, b'a', b'u', b't', b'h', 0xa5, b'a', b'u',
            b't', b'h', b'1', 0xa9, b'k', b'e', b'e', b'p', b'a', b'l', b'i', b'v', b'e', 0xc2,
        ];
        let helo = parse_helo(buf).unwrap();
        assert_eq!(helo.nonce, b"nonce1");
        assert_eq!(helo.auth_salt, b"auth1");
        assert!(!helo.keepalive);

        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x82, 0xa5, b'n', b'o', b'n', b'c', b'e', 0xa6,
            b'n', b'o', b'n', b'c', b'e', b'1', 0xa4, b'a', b'u', b't', b'h', 0xc4, 0x02, 0x04,
            0x05,
        ];
        let helo = parse_helo(buf).unwrap();
        assert_eq!(helo.nonce, b"nonce1");
        assert_eq!(helo.auth_salt, &[0x04, 0x05]);
        assert!(helo.keepalive);

        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x82, 0xa5, b'n', b'o', b'n', b'c', b'e', 0xa6,
            b'n', b'o', b'n', b'c', b'e', b'1', 0xa7, b'u', b'n', b'k', b'n', b'o', b'w', b'n',
            0xa5, b'v', b'a', b'l', b'u', b'e',
        ];
        let helo = parse_helo(buf).unwrap();
        assert_eq!(helo.nonce, b"nonce1");
        assert_eq!(helo.auth_salt, b"");
        assert!(helo.keepalive);
    }

    #[test]
    fn parse_helo_err() {
        let buf: &[u8] = &[0x91, 0xa4, b'H', b'E', b'L', b'O'];
        assert!(parse_helo(buf).is_err());

        let buf: &[u8] = &[0x92, 0xa5, b'H', b'E', b'L', b'L', b'O', 0x80];
        assert!(parse_helo(buf).is_err());

        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x81, 0xa5, b'n', b'o', b'n', b'c', b'e', 0xc4,
            0x05, 0x01, 0x02, 0x03,
        ];
        assert!(parse_helo(buf).is_err());

        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x81, 0xa5, b'n', b'o', b'n', b'c', b'e', 0x01,
        ];
        assert!(parse_helo(buf).is_err());

        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x81, 0xa4, b'a', b'u', b't', b'h', 0xc4, 0x10,
            0x01, 0x02,
        ];
        assert!(parse_helo(buf).is_err());

        let buf: &[u8] = &[
            0x92, 0xa4, b'H', b'E', b'L', b'O', 0x81, 0xa4, b'a', b'u', b't', b'h', 0x01,
        ];
        assert!(parse_helo(buf).is_err());
    }

    #[test]
    fn parse_pong_ok() {
        let buf: &[u8] = &[
            0x95, 0xa4, b'P', b'O', b'N', b'G', 0xc3, 0xa6, b'r', b'e', b'a', b's', b'o', b'n',
            0xa8, b'h', b'o', b's', b't', b'n', b'a', b'm', b'e', 0xa6, b'd', b'i', b'g', b'e',
            b's', b't',
        ];
        let pong = parse_pong(buf).unwrap();
        assert!(pong.auth_result);
        assert_eq!(pong.reason, "reason");
        assert_eq!(pong.server_hostname, "hostname");
        assert_eq!(pong.shared_key_digest, "digest");

        let buf: &[u8] = &[
            0x95, 0xa4, b'P', b'O', b'N', b'G', 0xc2, 0xa6, b'r', b'e', b'a', b's', b'o', b'n',
            0xa6, b's', b'e', b'r', b'v', b'e', b'r', 0xa6, b'd', b'i', b'g', b'e', b's', b't',
        ];
        let pong = parse_pong(buf).unwrap();
        assert!(!pong.auth_result);
        assert_eq!(pong.reason, "reason");
        assert_eq!(pong.server_hostname, "server");
        assert_eq!(pong.shared_key_digest, "digest");
    }

    #[test]
    fn parse_pong_err() {
        let buf: &[u8] = &[
            0x94, 0xa4, b'P', b'O', b'N', b'G', 0xc3, 0xa6, b'r', b'e', b'a', b's', b'o', b'n',
            0xa6, b's', b'e', b'r', b'v', b'e', b'r',
        ];
        assert!(parse_pong(buf).is_err());

        let buf: &[u8] = &[
            0x95, 0xa4, b'P', b'I', b'N', b'G', 0xc3, 0xa6, b'r', b'e', b'a', b's', b'o', b'n',
            0xa8, b'h', b'o', b's', b't', b'n', b'a', b'm', b'e', 0xa6, b'd', b'i', b'g', b'e',
            b's', b't',
        ];
        assert!(parse_pong(buf).is_err());

        let buf: &[u8] = &[
            0x95, 0xa4, b'P', b'O', b'N', b'G', 0xa4, b't', b'r', b'u', b'e', 0xa6, b'r', b'e',
            b'a', b's', b'o', b'n', 0xad, b's', b'e', b'r', b'v', b'e', b'r', b'.', b'e', b'x',
            b'a', b'm', b'p', b'l', b'e', 0xa6, b'd', b'i', b'g', b'e', b's', b't',
        ];
        assert!(parse_pong(buf).is_err());

        let buf: &[u8] = &[
            0x95, 0xa4, b'P', b'O', b'N', b'G', 0xc3, 0xc3, 0xad, b's', b'e', b'r', b'v', b'e',
            b'r', b'.', b'e', b'x', b'a', b'm', b'p', b'l', b'e', 0xa6, b'd', b'i', b'g', b'e',
            b's', b't',
        ];
        assert!(parse_pong(buf).is_err());
    }
}
