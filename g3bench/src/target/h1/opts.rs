/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use http::{HeaderValue, Request, header};

use g3_types::net::HttpAuth;

use crate::module::http::{AppendH1ConnectArgs, AppendHttpArgs, H1ConnectArgs, HttpClientArgs};

const HTTP_ARG_NO_KEEPALIVE: &str = "no-keepalive";
const HTTP_ARG_HEADER_SIZE: &str = "header-size";

pub(super) struct BenchHttpArgs {
    pub(super) common: HttpClientArgs,
    pub(super) connect: H1ConnectArgs,
    pub(super) no_keepalive: bool,
    pub(super) max_header_size: usize,
}

impl BenchHttpArgs {
    fn new(common: HttpClientArgs) -> Self {
        let connect = H1ConnectArgs::new(common.is_https());

        BenchHttpArgs {
            common,
            connect,
            no_keepalive: false,
            max_header_size: 4096,
        }
    }

    fn write_request_line<W: io::Write, R: Default>(
        &self,
        buf: &mut W,
        req: &Request<R>,
    ) -> io::Result<()> {
        write!(buf, "{} ", req.method())?;
        if self.connect.forward_proxy.is_some() {
            write!(
                buf,
                "{}://{}",
                self.common.target_url.scheme(),
                self.common.target
            )?;
        }
        match req.uri().path_and_query() {
            Some(v) => {
                buf.write_all(v.as_str().as_bytes())?;
            }
            None => {
                buf.write_all(b"/")?;
            }
        }
        buf.write_all(b" HTTP/1.1\r\n")?;

        Ok(())
    }

    pub(super) fn write_fixed_request_header<W: io::Write>(
        &self,
        buf: &mut W,
    ) -> anyhow::Result<()> {
        let mut static_request = self.common.build_static_request()?;

        if !static_request.headers().contains_key(header::HOST) {
            let v = HeaderValue::from_str(&self.common.target.to_string())?;
            static_request.headers_mut().insert(header::HOST, v);
        }

        if let Some(p) = &self.connect.forward_proxy {
            match &p.auth {
                HttpAuth::None => {}
                HttpAuth::Basic(basic) => {
                    if !static_request
                        .headers()
                        .contains_key(header::PROXY_AUTHORIZATION)
                    {
                        let value = HeaderValue::try_from(basic)
                            .map_err(|e| anyhow!("invalid auth value: {e:?}"))?;
                        static_request
                            .headers_mut()
                            .insert(header::PROXY_AUTHORIZATION, value);
                    }
                }
            }
        }

        if self.no_keepalive {
            static_request
                .headers_mut()
                .insert(header::CONNECTION, HeaderValue::from_static("close"));
        } else {
            static_request
                .headers_mut()
                .insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
        }

        self.write_request_line(buf, &static_request)?;

        for (k, v) in static_request.headers() {
            buf.write_all(k.as_str().as_bytes())?;
            buf.write_all(b": ")?;
            buf.write_all(v.as_bytes())?;
            buf.write_all(b"\r\n")?;
        }

        if !static_request.body().is_empty() {
            buf.write_all(b"\r\n")?;
            buf.write_all(static_request.body().as_slice())?;
        }
        Ok(())
    }
}

pub(super) fn add_http_args(app: Command) -> Command {
    app.arg(
        Arg::new(HTTP_ARG_NO_KEEPALIVE)
            .help("Disable http keepalive")
            .action(ArgAction::SetTrue)
            .long(HTTP_ARG_NO_KEEPALIVE),
    )
    .arg(
        Arg::new(HTTP_ARG_HEADER_SIZE)
            .value_name("SIZE")
            .help("Set max response header size")
            .long(HTTP_ARG_HEADER_SIZE)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .append_http_args()
    .append_h1_connect_args()
}

pub(super) fn parse_http_args(args: &ArgMatches) -> anyhow::Result<BenchHttpArgs> {
    let common = HttpClientArgs::parse_args(args)?;
    let mut h1_args = BenchHttpArgs::new(common);

    if args.get_flag(HTTP_ARG_NO_KEEPALIVE) {
        h1_args.no_keepalive = true;
    }

    if let Some(header_size) = g3_clap::humanize::get_usize(args, HTTP_ARG_HEADER_SIZE)? {
        h1_args.max_header_size = header_size;
    }

    h1_args
        .connect
        .parse_args(args)
        .context("invalid h1 connect args")?;

    match h1_args.common.target_url.scheme() {
        "http" | "https" => {}
        "ftp" => {
            if h1_args.connect.forward_proxy.is_none() {
                return Err(anyhow!(
                    "forward proxy is required for target url {}",
                    h1_args.common.target_url
                ));
            }
        }
        _ => {
            return Err(anyhow!(
                "unsupported target url {}",
                h1_args.common.target_url
            ));
        }
    }

    Ok(h1_args)
}
