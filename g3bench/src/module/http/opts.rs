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
const HTTP_ARG_PAYLOAD: &str = "payload";
const HTTP_ARG_NO_STRICT: &str = "no-strict";
const HTTP_ARG_BINARY_PAYLOAD: &str = "binary";

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
    pub(crate) body: Option<Vec<u8>>,

    pub(crate) target: UpstreamAddr,
    auth: HttpAuth,
    binary_payload: bool,
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
            body: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(15),
            target: upstream,
            auth,
            binary_payload: false,
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

        if let Some(payload) = self.payload() {
            if !req.headers().contains_key(http::header::CONTENT_LENGTH) {
                req.headers_mut().append(
                    http::header::CONTENT_LENGTH,
                    HeaderValue::from(payload.len()),
                );
            }
            if !req.headers().contains_key(http::header::CONTENT_TYPE) {
                let ctype = if self.binary_payload {
                    "application/octet-stream"
                } else {
                    "text/html; charset=utf-8"
                };
                req.headers_mut()
                    .append(http::header::CONTENT_TYPE, HeaderValue::from_static(ctype));
            }
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

    pub(crate) fn payload(&self) -> Option<&Vec<u8>> {
        self.body.as_ref()
    }

    pub(crate) fn parse_args(args: &ArgMatches) -> anyhow::Result<Self> {
        let url = if let Some(v) = args.get_one::<String>(HTTP_ARG_URL) {
            Url::parse(v).context(format!("invalid {HTTP_ARG_URL} value"))?
        } else {
            return Err(anyhow!("no target url set"));
        };

        let mut http_args = HttpClientArgs::new(url)?;

        let no_strict = args.get_flag(HTTP_ARG_NO_STRICT);
        http_args.binary_payload = args.get_flag(HTTP_ARG_BINARY_PAYLOAD);

        if let Some(v) = args.get_one::<String>(HTTP_ARG_METHOD) {
            let method = Method::from_str(v).context(format!("invalid {HTTP_ARG_METHOD} value"))?;
            http_args.method = method;
        }

        if let Ok(payload) = g3_clap::data::get(args, HTTP_ARG_PAYLOAD, http_args.binary_payload) {
            match http_args.method {
                Method::POST | Method::PUT => {
                    if !payload.is_empty() {
                        http_args.body = Some(payload);
                    }
                }
                _ => {
                    if !payload.is_empty() {
                        if no_strict {
                            http_args.body = Some(payload);
                        } else {
                            return Err(anyhow!(format!(
                                "--{HTTP_ARG_PAYLOAD} argument is only allowed for POST or PUT methods. \
                                Use --{HTTP_ARG_NO_STRICT} to ignore this check."
                            )));
                        }
                    }
                }
            }
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
                    .value_parser(["DELETE", "GET", "HEAD", "OPTIONS", "TRACE", "POST", "PUT"])
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
            .arg(
                Arg::new(HTTP_ARG_PAYLOAD)
                    .value_name("REQUEST BODY DATA")
                    .help("Request body payload data")
                    .long(HTTP_ARG_PAYLOAD)
                    .num_args(1),
            )
            .arg(
                Arg::new(HTTP_ARG_NO_STRICT)
                    .value_name("NO STRICT")
                    .long(HTTP_ARG_NO_STRICT)
                    .action(ArgAction::SetTrue)
                    .help("ignore HTTP method restrictions for payload data (--payload)"),
            )
            .arg(
                Arg::new(HTTP_ARG_BINARY_PAYLOAD)
                    .value_name("BINARY REQUEST PAYLOAD DATA")
                    .long(HTTP_ARG_BINARY_PAYLOAD)
                    .action(ArgAction::SetTrue)
                    .help("expect binary request payload data (--payload)"),
            )
    }
}
