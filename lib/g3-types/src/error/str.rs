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

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct FoundInvalidChar {
    position: usize,
    value: char,
}

impl FoundInvalidChar {
    pub fn new(position: usize, value: char) -> Self {
        FoundInvalidChar { position, value }
    }
}

impl fmt::Display for FoundInvalidChar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "found invalid char {} at position {}",
            self.value.escape_default(),
            self.position
        )
    }
}

impl Error for FoundInvalidChar {}
