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

use std::fmt;
use std::hash::{Hash, Hasher};

use super::{SelectiveHash, SelectiveItem};

pub struct WeightedValue<T> {
    value: T,
    weight: f64,
}

impl<T> WeightedValue<T> {
    pub const DEFAULT_WEIGHT: f64 = 1.0;

    pub fn new(value: T) -> Self {
        Self::with_weight(value, 1.0f64)
    }

    pub fn with_weight(value: T, weight: f64) -> Self {
        WeightedValue { value, weight }
    }

    #[inline]
    pub fn weight(&self) -> f64 {
        self.weight
    }

    #[inline]
    pub fn inner(&self) -> &T {
        &self.value
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> fmt::Debug for WeightedValue<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeightValue<T>")
            .field("value(T)", &self.value)
            .field("weight", &self.weight)
            .finish()
    }
}

impl<T: Default> Default for WeightedValue<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone> Clone for WeightedValue<T> {
    fn clone(&self) -> Self {
        WeightedValue {
            value: self.value.clone(),
            weight: self.weight,
        }
    }
}

impl<T: Copy> Copy for WeightedValue<T> {}

impl<T: PartialEq> PartialEq for WeightedValue<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(&other.value) && self.weight == other.weight
    }
}

impl<T: PartialEq + Eq> Eq for WeightedValue<T> {}

impl<T> SelectiveItem for WeightedValue<T> {
    #[inline]
    fn weight(&self) -> f64 {
        self.weight
    }
}

impl<T: Hash> SelectiveHash for WeightedValue<T> {
    #[inline]
    fn selective_hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}
