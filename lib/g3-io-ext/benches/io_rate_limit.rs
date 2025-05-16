/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#![feature(test)]

extern crate test;
use test::Bencher;

use std::num::NonZeroU32;
use std::time::Instant;

use governor::{Quota, RateLimiter, clock::DefaultClock, state::InMemoryState, state::NotKeyed};

use g3_io_ext::LocalStreamLimiter;

fn test_fixed_window(limiter: &mut LocalStreamLimiter, start: &Instant) {
    let ts = start.elapsed().as_millis() as u64;
    let _ = limiter.check(ts, 1);
}

fn test_fixed_window_3(limiter: &mut LocalStreamLimiter, start: &Instant) {
    let ts = start.elapsed().as_millis() as u64;
    let _ = limiter.check(ts, 3);
}

fn test_leaky_bucket(limiter: &RateLimiter<NotKeyed, InMemoryState, DefaultClock>) {
    let _ = limiter.check();
}

fn test_leaky_bucket_3(limiter: &RateLimiter<NotKeyed, InMemoryState, DefaultClock>) {
    let _ = limiter.check_n(unsafe { NonZeroU32::new_unchecked(3) });
}

#[bench]
fn fixed_window_ok1(b: &mut Bencher) {
    let start = Instant::now();
    let mut limiter = LocalStreamLimiter::new(10, 1024 * 1024 * 1024);
    b.iter(|| test_fixed_window(&mut limiter, &start));
}

#[bench]
fn fixed_window_ok3(b: &mut Bencher) {
    let start = Instant::now();
    let mut limiter = LocalStreamLimiter::new(10, 1024 * 1024 * 1024);
    b.iter(|| test_fixed_window_3(&mut limiter, &start));
}

#[bench]
fn fixed_window_empty(b: &mut Bencher) {
    let start = Instant::now();
    let mut limiter = LocalStreamLimiter::new(10, 1024);
    b.iter(|| test_fixed_window(&mut limiter, &start));
}

#[bench]
fn leaky_bucket_ok1(b: &mut Bencher) {
    let quota = Quota::per_second(unsafe { NonZeroU32::new_unchecked(1024 * 1024 * 1024) });
    let limiter = RateLimiter::direct(quota);
    b.iter(|| test_leaky_bucket(&limiter));
}

#[bench]
fn leaky_bucket_ok3(b: &mut Bencher) {
    let quota = Quota::per_second(unsafe { NonZeroU32::new_unchecked(1024 * 1024 * 1024) });
    let limiter = RateLimiter::direct(quota);
    b.iter(|| test_leaky_bucket_3(&limiter));
}

#[bench]
fn leaky_bucket_empty(b: &mut Bencher) {
    let quota = Quota::per_second(unsafe { NonZeroU32::new_unchecked(1024) });
    let limiter = RateLimiter::direct(quota);
    b.iter(|| test_leaky_bucket(&limiter));
}
