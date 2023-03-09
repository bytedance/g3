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

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct RequestStats {
    http_forward: AtomicU64,
    https_forward: AtomicU64,
    http_connect: AtomicU64,
    ftp_over_http: AtomicU64,
    socks_tcp_connect: AtomicU64,
    socks_udp_connect: AtomicU64,
    socks_udp_associate: AtomicU64,
}

#[derive(Default)]
pub(crate) struct RequestSnapshot {
    pub(crate) http_forward: u64,
    pub(crate) https_forward: u64,
    pub(crate) http_connect: u64,
    pub(crate) ftp_over_http: u64,
    pub(crate) socks_tcp_connect: u64,
    pub(crate) socks_udp_connect: u64,
    pub(crate) socks_udp_associate: u64,
}

impl RequestStats {
    pub(crate) fn add_http_forward(&self, is_https: bool) {
        if is_https {
            self.https_forward.fetch_add(1, Ordering::Relaxed);
        } else {
            self.http_forward.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn get_http_forward(&self) -> u64 {
        self.http_forward.load(Ordering::Relaxed)
    }

    pub(crate) fn get_https_forward(&self) -> u64 {
        self.https_forward.load(Ordering::Relaxed)
    }

    pub(crate) fn add_http_connect(&self) {
        self.http_connect.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_http_connect(&self) -> u64 {
        self.http_connect.load(Ordering::Relaxed)
    }

    pub(crate) fn add_ftp_over_http(&self) {
        self.ftp_over_http.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_ftp_over_http(&self) -> u64 {
        self.ftp_over_http.load(Ordering::Relaxed)
    }

    pub(crate) fn add_socks_tcp_connect(&self) {
        self.socks_tcp_connect.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_socks_tcp_connect(&self) -> u64 {
        self.socks_tcp_connect.load(Ordering::Relaxed)
    }

    pub(crate) fn add_socks_udp_connect(&self) {
        self.socks_udp_connect.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_socks_udp_connect(&self) -> u64 {
        self.socks_udp_connect.load(Ordering::Relaxed)
    }

    pub(crate) fn add_socks_udp_associate(&self) {
        self.socks_udp_associate.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_socks_udp_associate(&self) -> u64 {
        self.socks_udp_associate.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub(crate) struct KeepaliveRequestStats {
    http_forward: AtomicU64,
    https_forward: AtomicU64,
}

#[derive(Default)]
pub(crate) struct KeepaliveRequestSnapshot {
    pub(crate) http_forward: u64,
    pub(crate) https_forward: u64,
}

impl KeepaliveRequestStats {
    pub(crate) fn add_http_forward(&self, is_https: bool) {
        if is_https {
            self.https_forward.fetch_add(1, Ordering::Relaxed);
        } else {
            self.http_forward.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn get_http_forward(&self) -> u64 {
        self.http_forward.load(Ordering::Relaxed)
    }

    pub(crate) fn get_https_forward(&self) -> u64 {
        self.https_forward.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub(crate) struct RequestAliveStats {
    http_forward: AtomicI32,
    https_forward: AtomicI32,
    http_connect: AtomicI32,
    ftp_over_http: AtomicI32,
    socks_tcp_connect: AtomicI32,
    socks_udp_connect: AtomicI32,
    socks_udp_associate: AtomicI32,
}

impl RequestAliveStats {
    pub(crate) fn add_http_forward(&self, is_https: bool) {
        if is_https {
            self.https_forward.fetch_add(1, Ordering::Relaxed);
        } else {
            self.http_forward.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn del_http_forward(&self, is_https: bool) {
        if is_https {
            self.https_forward.fetch_sub(1, Ordering::Relaxed);
        } else {
            self.http_forward.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn get_http_forward(&self) -> i32 {
        self.http_forward.load(Ordering::Relaxed)
    }

    pub(crate) fn get_https_forward(&self) -> i32 {
        self.https_forward.load(Ordering::Relaxed)
    }

    pub(crate) fn add_http_connect(&self) {
        self.http_connect.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn del_http_connect(&self) {
        self.http_connect.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn get_http_connect(&self) -> i32 {
        self.http_connect.load(Ordering::Relaxed)
    }

    pub(crate) fn add_ftp_over_http(&self) {
        self.ftp_over_http.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn del_ftp_over_http(&self) {
        self.ftp_over_http.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn get_ftp_over_http(&self) -> i32 {
        self.ftp_over_http.load(Ordering::Relaxed)
    }

    pub(crate) fn add_socks_tcp_connect(&self) {
        self.socks_tcp_connect.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn del_socks_tcp_connect(&self) {
        self.socks_tcp_connect.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn get_socks_tcp_connect(&self) -> i32 {
        self.socks_tcp_connect.load(Ordering::Relaxed)
    }

    pub(crate) fn add_socks_udp_connect(&self) {
        self.socks_udp_connect.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn del_socks_udp_connect(&self) {
        self.socks_udp_connect.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn get_socks_udp_connect(&self) -> i32 {
        self.socks_udp_connect.load(Ordering::Relaxed)
    }

    pub(crate) fn add_socks_udp_associate(&self) {
        self.socks_udp_associate.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn del_socks_udp_associate(&self) {
        self.socks_udp_associate.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn get_socks_udp_associate(&self) -> i32 {
        self.socks_udp_associate.load(Ordering::Relaxed)
    }
}
