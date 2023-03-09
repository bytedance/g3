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
