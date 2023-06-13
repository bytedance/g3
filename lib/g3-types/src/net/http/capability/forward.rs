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
