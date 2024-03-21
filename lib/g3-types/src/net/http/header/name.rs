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

use std::borrow::Borrow;
use std::ops::Deref;

use smol_str::SmolStr;

#[derive(Clone)]
pub struct HttpOriginalHeaderName(SmolStr);

impl HttpOriginalHeaderName {
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'a> From<&'a str> for HttpOriginalHeaderName {
    fn from(value: &'a str) -> Self {
        HttpOriginalHeaderName(value.into())
    }
}

impl Borrow<str> for HttpOriginalHeaderName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for HttpOriginalHeaderName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
