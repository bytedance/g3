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

use foreign_types::ForeignTypeRef;
use openssl::x509::X509Ref;

pub trait X509Ext {
    fn pathlen(&self) -> Option<u32>;
}

impl X509Ext for X509Ref {
    fn pathlen(&self) -> Option<u32> {
        let v = unsafe { super::ffi::X509_get_pathlen(self.as_ptr()) };
        u32::try_from(v).ok()
    }
}
