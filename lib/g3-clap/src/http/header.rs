/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use clap::ArgMatches;
use http::{HeaderName, HeaderValue};

pub fn get_headers(args: &ArgMatches, id: &str) -> anyhow::Result<Vec<(HeaderName, HeaderValue)>> {
    let mut headers = Vec::new();
    if let Some(v) = args.get_many::<String>(id) {
        for s in v {
            let Some((name, value)) = s.split_once(':') else {
                return Err(anyhow!("invalid HTTP header: {s}"));
            };
            let name = HeaderName::from_str(name)
                .map_err(|e| anyhow!("invalid HTTP header name {name}: {e}"))?;
            let value = HeaderValue::from_str(value)
                .map_err(|e| anyhow!("invalid HTTP header value {value}: {e}"))?;
            headers.push((name, value));
        }
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction, Command};
    use http::header;

    // Helper function to create ArgMatches for testing
    fn create_args(headers: &[&str]) -> ArgMatches {
        Command::new("test")
            .arg(
                Arg::new("headers")
                    .long("headers")
                    .num_args(0..)
                    .action(ArgAction::Append),
            )
            .get_matches_from(
                std::iter::once("test").chain(headers.iter().flat_map(|h| ["--headers", h])),
            )
    }

    #[test]
    fn get_headers_ok() {
        // No headers provided should return empty vector
        let args = create_args(&[]);
        assert!(get_headers(&args, "headers").unwrap().is_empty());

        // Single valid header
        let args = create_args(&["Content-Type:text/html"]);
        let result = get_headers(&args, "headers").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, header::CONTENT_TYPE);
        assert_eq!(result[0].1, "text/html");

        // Multiple valid headers
        let args = create_args(&["Accept:*/*", "User-Agent:TestAgent"]);
        let result = get_headers(&args, "headers").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "accept");
        assert_eq!(result[0].1, "*/*");
        assert_eq!(result[1].0, "user-agent");
        assert_eq!(result[1].1, "TestAgent");

        // Header with empty value
        let args = create_args(&["X-Empty-Header:"]);
        let result = get_headers(&args, "headers").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "x-empty-header");
        assert_eq!(result[0].1, "");
    }

    #[test]
    fn get_headers_err() {
        // Missing colon in header format
        let args = create_args(&["InvalidHeader"]);
        assert!(get_headers(&args, "headers").is_err());

        // Invalid header name
        let args = create_args(&["Invalid@Name:value"]);
        assert!(get_headers(&args, "headers").is_err());

        // Invalid header value (contains newline)
        let args = create_args(&["X-Test:invalid\nvalue"]);
        assert!(get_headers(&args, "headers").is_err());
    }
}
