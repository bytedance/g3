/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::Context;
use clap::{ArgMatches, Command};
use http::Version;

use crate::module::http::{AppendH1ConnectArgs, H1ConnectArgs};
use crate::target::websocket::{AppendWebsocketArgs, WebsocketArgs};

pub(super) struct H1WebsocketArgs {
    pub(super) common: WebsocketArgs,
    pub(super) connect: H1ConnectArgs,
}

impl H1WebsocketArgs {
    fn new(common: WebsocketArgs) -> Self {
        let connect = H1ConnectArgs::new(common.is_https());

        H1WebsocketArgs { common, connect }
    }

    pub(super) fn build_upgrade_request(&self, buf: &mut Vec<u8>) -> anyhow::Result<[u8; 16]> {
        let (req, key) = self.common.build_upgrade_request(Version::HTTP_11)?;

        buf.extend_from_slice(b"GET ");
        buf.extend_from_slice(self.common.target_url.path().as_bytes());
        if let Some(query) = self.common.target_url.query() {
            buf.push(b'?');
            buf.extend_from_slice(query.as_bytes());
        }
        buf.extend_from_slice(b" HTTP/1.1\r\n");
        buf.extend_from_slice(b"Upgrade: websocket\r\n");
        buf.extend_from_slice(b"Connection: Upgrade\r\n");
        if !req.headers().contains_key(http::header::HOST) {
            buf.extend_from_slice(b"Host: ");
            let host = self.common.target.to_string();
            buf.extend_from_slice(host.as_bytes());
            buf.extend_from_slice(b"\r\n");
        }
        for (k, v) in req.headers() {
            buf.extend_from_slice(k.as_str().as_bytes());
            buf.extend_from_slice(b": ");
            buf.extend_from_slice(v.as_bytes());
            buf.extend_from_slice(b"\r\n");
        }

        buf.extend_from_slice(b"\r\n");
        Ok(key)
    }
}

pub(super) fn add_h1_websocket_args(app: Command) -> Command {
    app.append_websocket_args().append_h1_connect_args()
}

pub(super) fn parse_h1_websocket_args(args: &ArgMatches) -> anyhow::Result<H1WebsocketArgs> {
    let common = WebsocketArgs::parse_args(args)?;
    let mut h1_websocket_args = H1WebsocketArgs::new(common);

    h1_websocket_args
        .connect
        .parse_args(args)
        .context("invalid h1 connect args")?;

    Ok(h1_websocket_args)
}
