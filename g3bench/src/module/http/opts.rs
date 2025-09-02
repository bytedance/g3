/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use http::{HeaderName, HeaderValue, Method, Request, StatusCode, Version};
use url::Url;

use g3_types::net::{HttpAuth, UpstreamAddr};

const HTTP_ARG_URL: &str = "url";
const HTTP_ARG_METHOD: &str = "method";
const HTTP_ARG_HEADER: &str = "header";
const HTTP_ARG_OK_STATUS: &str = "ok-status";
const HTTP_ARG_TIMEOUT: &str = "timeout";
const HTTP_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";

pub(crate) trait AppendHttpArgs {
    fn append_http_args(self) -> Self;
}

pub(crate) struct HttpClientArgs {
    pub(crate) method: Method,
    pub(crate) target_url: Url,
    headers: Vec<(HeaderName, HeaderValue)>,
    pub(crate) ok_status: Option<StatusCode>,
    pub(crate) timeout: Duration,
    pub(crate) connect_timeout: Duration,

    pub(crate) target: UpstreamAddr,
    auth: HttpAuth,
}

impl HttpClientArgs {
    fn new(url: Url) -> anyhow::Result<Self> {
        let upstream = UpstreamAddr::try_from(&url)?;
        let auth = HttpAuth::try_from(&url)
            .map_err(|e| anyhow!("failed to detect upstream auth method: {e}"))?;
        Ok(HttpClientArgs {
            method: Method::GET,
            target_url: url,
            headers: Vec::new(),
            ok_status: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(15),
            target: upstream,
            auth,
        })
    }

    pub(crate) fn is_https(&self) -> bool {
        self.target_url.scheme() == "https"
    }

    pub(crate) fn build_static_request(&self, version: Version) -> anyhow::Result<Request<()>> {
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
            .method(self.method.clone())
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

        Ok(req)
    }

    pub(crate) fn parse_args(args: &ArgMatches) -> anyhow::Result<Self> {
        let url = if let Some(v) = args.get_one::<String>(HTTP_ARG_URL) {
            Url::parse(v).context(format!("invalid {HTTP_ARG_URL} value"))?
        } else {
            return Err(anyhow!("no target url set"));
        };
        let mut http_args = HttpClientArgs::new(url)?;

        if let Some(v) = args.get_one::<String>(HTTP_ARG_METHOD) {
            let method = Method::from_str(v).context(format!("invalid {HTTP_ARG_METHOD} value"))?;
            http_args.method = method;
        }
        http_args.headers = g3_clap::http::get_headers(args, HTTP_ARG_HEADER)?;
        if let Some(code) = args.get_one::<StatusCode>(HTTP_ARG_OK_STATUS) {
            http_args.ok_status = Some(*code);
        }
        if let Some(timeout) = g3_clap::humanize::get_duration(args, HTTP_ARG_TIMEOUT)? {
            http_args.timeout = timeout;
        }
        if let Some(timeout) = g3_clap::humanize::get_duration(args, HTTP_ARG_CONNECT_TIMEOUT)? {
            http_args.connect_timeout = timeout;
        }

        Ok(http_args)
    }
}

impl AppendHttpArgs for Command {
    fn append_http_args(self) -> Self {
        self.arg(Arg::new(HTTP_ARG_URL).required(true).num_args(1))
            .arg(
                Arg::new(HTTP_ARG_METHOD)
                    .value_name("METHOD")
                    .short('m')
                    .long(HTTP_ARG_METHOD)
                    .num_args(1)
                    .value_parser(["DELETE", "GET", "HEAD", "OPTIONS", "TRACE"])
                    .default_value("GET"),
            )
            .arg(
                Arg::new(HTTP_ARG_HEADER)
                    .value_name("HEADER")
                    .short('H')
                    .long(HTTP_ARG_HEADER)
                    .action(ArgAction::Append),
            )
            .arg(
                Arg::new(HTTP_ARG_OK_STATUS)
                    .help("Only treat this status code as success")
                    .value_name("STATUS CODE")
                    .long(HTTP_ARG_OK_STATUS)
                    .num_args(1)
                    .value_parser(value_parser!(StatusCode)),
            )
            .arg(
                Arg::new(HTTP_ARG_TIMEOUT)
                    .value_name("TIMEOUT DURATION")
                    .help("Http response timeout")
                    .default_value("30s")
                    .long(HTTP_ARG_TIMEOUT)
                    .num_args(1),
            )
            .arg(
                Arg::new(HTTP_ARG_CONNECT_TIMEOUT)
                    .value_name("TIMEOUT DURATION")
                    .help("Timeout for connection to next peer")
                    .default_value("15s")
                    .long(HTTP_ARG_CONNECT_TIMEOUT)
                    .num_args(1),
            )
    }
}
