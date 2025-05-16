/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::ops;

use http::Method;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HttpForwardCapability {
    forward_https: bool,
    forward_ftp_all: bool,
    forward_ftp_get: bool,
    forward_ftp_put: bool,
    forward_ftp_del: bool,
}

impl HttpForwardCapability {
    #[inline]
    pub fn set_forward_https(&mut self, enable: bool) {
        self.forward_https = enable;
    }

    #[inline]
    pub fn forward_https(&self) -> bool {
        self.forward_https
    }

    pub fn set_forward_ftp_all(&mut self, enable: bool) {
        self.forward_ftp_all = enable;
        self.set_forward_ftp_get(enable);
        self.set_forward_ftp_put(enable);
        self.set_forward_ftp_del(enable);
    }

    #[inline]
    pub fn set_forward_ftp_get(&mut self, enable: bool) {
        self.forward_ftp_get = enable;
    }

    #[inline]
    pub fn set_forward_ftp_put(&mut self, enable: bool) {
        self.forward_ftp_put = enable;
    }

    #[inline]
    pub fn set_forward_ftp_del(&mut self, enable: bool) {
        self.forward_ftp_del = enable;
    }

    pub fn forward_ftp(&self, method: &Method) -> bool {
        match *method {
            Method::GET => self.forward_ftp_get,
            Method::PUT => self.forward_ftp_put,
            Method::DELETE => self.forward_ftp_del,
            _ => self.forward_ftp_all,
        }
    }
}

impl ops::BitAnd for HttpForwardCapability {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        HttpForwardCapability {
            forward_https: self.forward_https & rhs.forward_https,
            forward_ftp_all: self.forward_ftp_all & rhs.forward_ftp_all,
            forward_ftp_get: self.forward_ftp_get & rhs.forward_ftp_get,
            forward_ftp_put: self.forward_ftp_put & rhs.forward_ftp_put,
            forward_ftp_del: self.forward_ftp_del & rhs.forward_ftp_del,
        }
    }
}
