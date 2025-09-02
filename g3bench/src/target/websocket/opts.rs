/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::{Context, anyhow};
use base64::prelude::*;
use bytes::Bytes;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Version};
use openssl::error::ErrorStack;
use openssl::md::Md;
use openssl::md_ctx::MdCtx;
use url::Url;

use g3_types::net::{HttpAuth, UpstreamAddr};

use super::{ClientFrameBuilder, FrameType};

const WEBSOCKET_ARG_URL: &str = "url";
const WEBSOCKET_ARG_HEADER: &str = "header";
const WEBSOCKET_ARG_TIMEOUT: &str = "timeout";
const WEBSOCKET_ARG_UPGRADE_TIMEOUT: &str = "upgrade-timeout";
const WEBSOCKET_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";
const WEBSOCKET_ARG_PROTOCOL: &str = "protocol";
const WEBSOCKET_ARG_PAYLOAD: &str = "payload";
const WEBSOCKET_ARG_BINARY: &str = "binary";
const WEBSOCKET_ARG_MAX_FRAME_SIZE: &str = "max-frame-size";
const WEBSOCKET_ARG_CHECK_MESSAGE_LENGTH: &str = "check-message-length";

const WEBSOCKET_KEY_SIZE: usize = 16;
const WEBSOCKET_ACCEPT_SIZE: usize = 20;

pub(crate) trait AppendWebsocketArgs {
    fn append_websocket_args(self) -> Self;
}

pub(super) struct WebsocketArgs {
    pub(super) target_url: Url,
    headers: Vec<(HeaderName, HeaderValue)>,
    pub(super) timeout: Duration,
    pub(super) upgrade_timeout: Duration,
    pub(super) connect_timeout: Duration,
    pub(super) protocol: Option<HeaderValue>,
    binary: bool,
    payload: Vec<u8>,
    pub(crate) max_frame_size: usize,
    check_message_length: Option<usize>,

    pub(super) target: UpstreamAddr,
    pub(super) auth: HttpAuth,
}

impl WebsocketArgs {
    fn new(url: Url) -> anyhow::Result<Self> {
        if !matches!(url.scheme(), "ws" | "wss") {
            return Err(anyhow!("unsupported websocket url {url}"));
        }
        let upstream = UpstreamAddr::try_from(&url)?;
        let auth = HttpAuth::try_from(&url)
            .map_err(|e| anyhow!("failed to detect upstream auth method: {e}"))?;
        Ok(WebsocketArgs {
            target_url: url,
            headers: Vec::new(),
            timeout: Duration::from_secs(30),
            upgrade_timeout: Duration::from_secs(4),
            connect_timeout: Duration::from_secs(15),
            protocol: None,
            binary: false,
            max_frame_size: 4 * 1024 * 1024,
            check_message_length: None,
            payload: Vec::new(),
            target: upstream,
            auth,
        })
    }

    pub(super) fn is_https(&self) -> bool {
        self.target_url.scheme() == "wss"
    }

    pub(super) fn build_upgrade_request(
        &self,
        version: Version,
    ) -> anyhow::Result<(Request<()>, [u8; WEBSOCKET_KEY_SIZE])> {
        let path_and_query = if let Some(q) = self.target_url.query() {
            format!("{}?{q}", self.target_url.path())
        } else {
            self.target_url.path().to_string()
        };
        let uri = http::Uri::builder()
            .scheme(self.target_url.scheme())
            .authority(self.target.to_string())
            .path_and_query(path_and_query)
            .build()
            .map_err(|e| anyhow!("failed to build request: {e:?}"))?;

        let mut req = Request::builder()
            .version(version)
            .method(Method::GET)
            .uri(uri)
            .body(())
            .map_err(|e| anyhow!("failed to build request: {e:?}"))?;

        for (key, value) in self.headers.iter() {
            req.headers_mut().append(key, value.clone());
        }

        if !req.headers().contains_key(http::header::AUTHORIZATION) {
            match &self.auth {
                HttpAuth::None => {}
                HttpAuth::Basic(basic) => {
                    let value = HeaderValue::try_from(basic)
                        .map_err(|e| anyhow!("invalid auth value: {e:?}"))?;
                    req.headers_mut().insert(http::header::AUTHORIZATION, value);
                }
            }
        }

        req.headers_mut().insert(
            http::header::SEC_WEBSOCKET_VERSION,
            HeaderValue::from_static("13"),
        );
        if let Some(v) = &self.protocol {
            req.headers_mut()
                .insert(http::header::SEC_WEBSOCKET_PROTOCOL, v.clone());
        }
        let mut key = [0u8; WEBSOCKET_KEY_SIZE];
        fastrand::fill(&mut key);
        let encoded_key = Bytes::from(BASE64_STANDARD.encode(key));
        req.headers_mut()
            .insert(http::header::SEC_WEBSOCKET_KEY, unsafe {
                HeaderValue::from_maybe_shared_unchecked(encoded_key)
            });

        Ok((req, key))
    }

    fn expected_sum(
        key: [u8; WEBSOCKET_KEY_SIZE],
    ) -> Result<[u8; WEBSOCKET_ACCEPT_SIZE], ErrorStack> {
        let mut md = MdCtx::new()?;
        md.digest_init(Md::sha1())?;
        let mut encoded_key = [0u8; WEBSOCKET_KEY_SIZE * 2];
        let len = BASE64_STANDARD
            .encode_slice(key, encoded_key.as_mut_slice())
            .unwrap();
        md.digest_update(&encoded_key[..len])?;
        md.digest_update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11")?;
        let mut expected_sum = [0u8; WEBSOCKET_ACCEPT_SIZE];
        md.digest_final(&mut expected_sum)?;
        Ok(expected_sum)
    }

    pub(super) fn verify_upgrade_response_headers(
        &self,
        key: [u8; WEBSOCKET_KEY_SIZE],
        headers: HeaderMap,
    ) -> anyhow::Result<()> {
        let Some(accept_str) = headers.get(http::header::SEC_WEBSOCKET_ACCEPT) else {
            return Err(anyhow!("no Sec-WebSocket-Accept header found"));
        };
        let mut accept_sum = [0u8; WEBSOCKET_ACCEPT_SIZE];
        BASE64_STANDARD
            .decode_slice(accept_str.as_bytes(), &mut accept_sum)
            .map_err(|e| {
                anyhow!(
                    "the value of Sec-WebSocket-Accept is not a valid base64 encoding of sha1 sum: {e}"
                )
            })?;

        let expected_sum = Self::expected_sum(key)
            .map_err(|e| anyhow!("failed to calc expected Sec-WebSocket-Accept value: {e}"))?;
        if accept_sum != expected_sum {
            return Err(anyhow!(
                "Sec-WebSocket-Accept sum value mismatch: expected {expected_sum:?} but get {accept_sum:?}"
            ));
        }

        if let Some(expected_p) = &self.protocol {
            let Some(actual_p) = headers.get(http::header::SEC_WEBSOCKET_PROTOCOL) else {
                return Err(anyhow!("no Sec-WebSocket-Protocol header set"));
            };
            if expected_p != actual_p {
                return Err(anyhow!("Sec-WebSocket-Protocol value mismatch"));
            }
        }

        Ok(())
    }

    pub(super) fn verify_response_data(
        &self,
        frame_type: FrameType,
        data: &[u8],
    ) -> anyhow::Result<()> {
        match frame_type {
            FrameType::Continue | FrameType::Close | FrameType::Ping => unreachable!(),
            FrameType::Pong => return Err(anyhow!("unexpected Pong response frame")),
            FrameType::Binary => {
                if !self.binary {
                    return Err(anyhow!("unexpected Binary response frame"));
                }
            }
            FrameType::Text => {
                if self.binary {
                    return Err(anyhow!("unexpected Text response frame"));
                }
            }
        }

        if let Some(l) = self.check_message_length
            && data.len() != l
        {
            return Err(anyhow!(
                "The expected response message length is {l} but we get {}",
                data.len()
            ));
        }

        Ok(())
    }

    pub(super) fn build_request_frames(&self, buf: &mut Vec<u8>) {
        let frame_type = if self.binary {
            FrameType::Binary
        } else {
            FrameType::Text
        };
        let builder = ClientFrameBuilder::new(frame_type, self.max_frame_size);
        builder.build_frames(&self.payload, buf);
    }

    pub(crate) fn parse_args(args: &ArgMatches) -> anyhow::Result<Self> {
        let url = if let Some(v) = args.get_one::<String>(WEBSOCKET_ARG_URL) {
            Url::parse(v).context(format!("invalid {WEBSOCKET_ARG_URL} value"))?
        } else {
            return Err(anyhow!("no target url set"));
        };
        let mut websocket_args = WebsocketArgs::new(url)?;
        websocket_args.headers = g3_clap::http::get_headers(args, WEBSOCKET_ARG_HEADER)?;

        if let Some(timeout) = g3_clap::humanize::get_duration(args, WEBSOCKET_ARG_TIMEOUT)? {
            websocket_args.timeout = timeout;
        }
        if let Some(timeout) = g3_clap::humanize::get_duration(args, WEBSOCKET_ARG_UPGRADE_TIMEOUT)?
        {
            websocket_args.upgrade_timeout = timeout;
        }
        if let Some(timeout) = g3_clap::humanize::get_duration(args, WEBSOCKET_ARG_CONNECT_TIMEOUT)?
        {
            websocket_args.connect_timeout = timeout;
        }

        if let Some(p) = args.get_one::<String>(WEBSOCKET_ARG_PROTOCOL) {
            let v = HeaderValue::from_bytes(p.as_bytes()).context("invalid websocket protocol")?;
            websocket_args.protocol = Some(v);
        }

        websocket_args.binary = args.get_flag(WEBSOCKET_ARG_BINARY);
        websocket_args.payload =
            g3_clap::data::get(args, WEBSOCKET_ARG_PAYLOAD, websocket_args.binary)?;

        if let Some(max_frame_size) =
            g3_clap::humanize::get_usize(args, WEBSOCKET_ARG_MAX_FRAME_SIZE)?
        {
            websocket_args.max_frame_size = max_frame_size;
        }

        websocket_args.check_message_length = args
            .get_one::<usize>(WEBSOCKET_ARG_CHECK_MESSAGE_LENGTH)
            .copied();

        Ok(websocket_args)
    }
}

impl AppendWebsocketArgs for Command {
    fn append_websocket_args(self) -> Self {
        self.arg(Arg::new(WEBSOCKET_ARG_URL).required(true).num_args(1))
            .arg(
                Arg::new(WEBSOCKET_ARG_HEADER)
                    .value_name("HEADER")
                    .short('H')
                    .long(WEBSOCKET_ARG_HEADER)
                    .action(ArgAction::Append),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_TIMEOUT)
                    .value_name("TIMEOUT DURATION")
                    .help("Websocket response timeout")
                    .default_value("30s")
                    .long(WEBSOCKET_ARG_TIMEOUT)
                    .num_args(1),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_UPGRADE_TIMEOUT)
                    .value_name("TIMEOUT DURATION")
                    .help("Timeout for upgrade http connection to websocket")
                    .default_value("4s")
                    .long(WEBSOCKET_ARG_UPGRADE_TIMEOUT)
                    .num_args(1),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_CONNECT_TIMEOUT)
                    .value_name("TIMEOUT DURATION")
                    .help("Timeout for connection to next peer")
                    .default_value("15s")
                    .long(WEBSOCKET_ARG_CONNECT_TIMEOUT)
                    .num_args(1),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_PROTOCOL)
                    .value_name("WEBSOCKET PROTOCOL")
                    .help("Set the websocket protocol and check it")
                    .long(WEBSOCKET_ARG_PROTOCOL)
                    .num_args(1),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_BINARY)
                    .help("Send binary frame instead of text frame")
                    .action(ArgAction::SetTrue)
                    .long(WEBSOCKET_ARG_BINARY),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_PAYLOAD)
                    .help("Frame payload")
                    .required(true)
                    .long(WEBSOCKET_ARG_PAYLOAD)
                    .num_args(1),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_MAX_FRAME_SIZE)
                    .help("Set max frame size")
                    .long(WEBSOCKET_ARG_MAX_FRAME_SIZE)
                    .default_value("1M")
                    .num_args(1),
            )
            .arg(
                Arg::new(WEBSOCKET_ARG_CHECK_MESSAGE_LENGTH)
                    .help("Check the message length in response")
                    .long(WEBSOCKET_ARG_CHECK_MESSAGE_LENGTH)
                    .num_args(1)
                    .value_parser(value_parser!(usize)),
            )
    }
}
