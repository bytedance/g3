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

#![feature(test)]

extern crate test;
use test::Bencher;

use std::str::FromStr;

use http::{HeaderName, HeaderValue};

use g3_http::{HttpHeaderLine, HttpStatusLine};

fn simple_parse_header_line(line: &[u8]) -> (HeaderName, HeaderValue) {
    let header = HttpHeaderLine::parse(line).unwrap();
    let name = HeaderName::from_str(header.name).unwrap();
    let value = HeaderValue::from_str(header.value).unwrap();
    (name, value)
}

fn simple_parse_status_line(line: &[u8]) -> u16 {
    let status = HttpStatusLine::parse(line).unwrap();
    status.code
}

fn http_parse_header_line(line: &[u8]) -> (HeaderName, HeaderValue) {
    let mut headers = [httparse::EMPTY_HEADER; 1];
    let _ = httparse::parse_headers(line, &mut headers).unwrap();
    let header = headers[0];
    let name = HeaderName::from_str(header.name).unwrap();
    let value = HeaderValue::from_bytes(header.value).unwrap();
    (name, value)
}

fn http_parse_status_line(line: &[u8]) -> u16 {
    let mut headers = [httparse::EMPTY_HEADER; 1];
    let mut rsp = httparse::Response::new(&mut headers);
    rsp.parse(line).unwrap();
    rsp.code.unwrap()
}

#[bench]
fn simple_parse_header_short(b: &mut Bencher) {
    b.iter(|| simple_parse_header_line(b"Host: www.microsoft.com\r\n"));
}

#[bench]
fn simple_parse_header_medium(b: &mut Bencher) {
    b.iter(|| simple_parse_header_line(b"Authorization: Basic ZGVtb191c2VyOjEyMzQ1Njc4OTBxd2VydHl1aW9wYXNkZmdoamtsenhjdmJubQo=\r\n"));
}

#[bench]
fn simple_parse_header_long(b: &mut Bencher) {
    b.iter(|| simple_parse_header_line(b"Proxy-Authorization: Basic ZGVtb191c2VyOjEyMzQ1Njc4OTBxd2VydHl1aW9wYXNkZmdoamtsenhjdmJubQo=\r\n"));
}

#[bench]
fn simple_parse_status(b: &mut Bencher) {
    b.iter(|| simple_parse_status_line(b"HTTP/1.1 200 OK\r\n"));
}

#[bench]
fn http_parse_header_short(b: &mut Bencher) {
    b.iter(|| http_parse_header_line(b"Host: www.microsoft.com\r\n"));
}

#[bench]
fn http_parse_header_medium(b: &mut Bencher) {
    b.iter(|| http_parse_header_line(b"Authorization: Basic ZGVtb191c2VyOjEyMzQ1Njc4OTBxd2VydHl1aW9wYXNkZmdoamtsenhjdmJubQo=\r\n"));
}

#[bench]
fn http_parse_header_long(b: &mut Bencher) {
    b.iter(|| http_parse_header_line(b"Proxy-Authorization: Basic ZGVtb191c2VyOjEyMzQ1Njc4OTBxd2VydHl1aW9wYXNkZmdoamtsenhjdmJubQo=\r\n"));
}

#[bench]
fn http_parse_status(b: &mut Bencher) {
    b.iter(|| http_parse_status_line(b"HTTP/1.1 200 OK\r\n"));
}
