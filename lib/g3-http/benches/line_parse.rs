/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
