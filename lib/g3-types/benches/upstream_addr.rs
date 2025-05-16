/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#![feature(test)]

extern crate test;
use test::Bencher;

use std::str::FromStr;

use g3_types::net::UpstreamAddr;

#[bench]
fn ipv4_ip(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("127.0.0.1"));
}

#[bench]
fn ipv4_addr(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("127.0.0.1:80"));
}

#[bench]
fn ipv6_ip(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("2001:db8::1"));
}

#[bench]
fn ipv6_squired_ip(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("[2001:db8::1]"));
}

#[bench]
fn ipv6_addr(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("[2001:db8::1]:80"));
}

#[bench]
fn ipv6_mapped_ipv4_ip(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("::ffff:192.168.89.9"));
}

#[bench]
fn ipv6_mapped_ipv4_squired_ip(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("[::ffff:192.168.89.9]"));
}

#[bench]
fn ipv6_mapped_ipv4_addr(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("[::ffff:192.168.89.9]:80"));
}

#[bench]
fn domain_t(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("test.example.net"));
}

#[bench]
fn domain_t_with_port(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("test.example.net:80"));
}

#[bench]
fn domain_f(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("face.example.net"));
}

#[bench]
fn domain_f_with_port(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("face.example.net:80"));
}

#[bench]
fn domain_1(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("1test.example.net"));
}

#[bench]
fn domain_1_with_port(b: &mut Bencher) {
    b.iter(|| UpstreamAddr::from_str("test.example.net:80"));
}
