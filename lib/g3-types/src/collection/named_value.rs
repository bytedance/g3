/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
