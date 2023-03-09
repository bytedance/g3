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

use http::Uri;
use percent_encoding::percent_decode_str;
use std::borrow::Cow;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FtpRequestPath {
    DefaultDir(&'static str),
    ListOnly(String),
    FileOnly(String),
    AutoDetect(String),
}

impl FtpRequestPath {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            FtpRequestPath::DefaultDir(s) => s,
            FtpRequestPath::ListOnly(s) => s,
            FtpRequestPath::FileOnly(s) => s,
            FtpRequestPath::AutoDetect(s) => s,
        }
    }

    #[inline]
    pub(crate) fn detect_fact(&self) -> bool {
        matches!(
            self,
            FtpRequestPath::AutoDetect(_) | FtpRequestPath::FileOnly(_)
        )
    }
}

impl From<&Uri> for FtpRequestPath {
    fn from(uri: &Uri) -> Self {
        let raw = uri.path();

        let (path, param) = if let Some(p) = memchr::memchr(b';', raw.as_bytes()) {
            // see rfc1738
            (&raw[0..p], &raw[p + 1..])
        } else {
            (raw, "")
        };

        let path = path
            .split('/')
            .skip_while(|v| (*v).is_empty())
            .map(|s| percent_decode_str(s).decode_utf8_lossy())
            .map(|s| if s == "/" { Cow::Borrowed("") } else { s })
            .collect::<Vec<Cow<str>>>()
            .join("/");
        if path.is_empty() {
            return FtpRequestPath::DefaultDir(".");
        }

        match param {
            "type=a" | "type=i" => FtpRequestPath::FileOnly(path),
            "type=d" => FtpRequestPath::ListOnly(path),
            _ => {
                if path.as_bytes().ends_with(b"/") {
                    FtpRequestPath::ListOnly(
                        path.strip_suffix('/')
                            .map(|s| s.to_string())
                            .unwrap_or(path),
                    )
                } else {
                    FtpRequestPath::AutoDetect(path)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_uri() {
        let uri = Uri::from_static("http://127.0.0.1");
        assert_eq!(FtpRequestPath::from(&uri), FtpRequestPath::DefaultDir("."));

        let uri = Uri::from_static("http://127.0.0.1/");
        assert_eq!(FtpRequestPath::from(&uri), FtpRequestPath::DefaultDir("."));

        let uri = Uri::from_static("http://127.0.0.1/a/");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::ListOnly("a".to_string())
        );

        let uri = Uri::from_static("http://127.0.0.1/%E6%B5%8B%E8%AF%95/");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::ListOnly("测试".to_string())
        );

        let uri = Uri::from_static("http://127.0.0.1/%2f/a/");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::ListOnly("/a".to_string())
        );

        let uri = Uri::from_static("http://127.0.0.1/%2F/%E6%B5%8B%E8%AF%95/");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::ListOnly("/测试".to_string())
        );

        let uri = Uri::from_static("http://127.0.0.1/a");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::AutoDetect("a".to_string())
        );

        let uri = Uri::from_static("http://127.0.0.1/%E6%B5%8B%E8%AF%95");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::AutoDetect("测试".to_string())
        );

        let uri = Uri::from_static("http://127.0.0.1/%2f/a");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::AutoDetect("/a".to_string())
        );

        let uri = Uri::from_static("http://127.0.0.1/%2F/%E6%B5%8B%E8%AF%95");
        assert_eq!(
            FtpRequestPath::from(&uri),
            FtpRequestPath::AutoDetect("/测试".to_string())
        );
    }
}
