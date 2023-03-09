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

pub trait NamedValue {
    type Name: ?Sized;
    type NameOwned: Borrow<Self::Name>;

    fn name(&self) -> &Self::Name;
    fn name_owned(&self) -> Self::NameOwned;
}

impl NamedValue for String {
    type Name = str;
    type NameOwned = String;

    fn name(&self) -> &Self::Name {
        self.as_str()
    }

    fn name_owned(&self) -> Self::NameOwned {
        self.to_string()
    }
}
