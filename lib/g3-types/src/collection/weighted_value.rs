/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::hash::{Hash, Hasher};

use super::SelectiveItem;

pub struct WeightedValue<T> {
    value: T,
    weight: f64,
}

impl<T> WeightedValue<T> {
    pub const DEFAULT_WEIGHT: f64 = 1.0;

    pub fn new(value: T) -> Self {
        Self::with_weight(value, Self::DEFAULT_WEIGHT)
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

impl<T: Hash> SelectiveItem for WeightedValue<T> {
    #[inline]
    fn weight(&self) -> f64 {
        self.weight
    }

    #[inline]
    fn selective_hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}
