/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

pub trait OptionExt {
    #[must_use]
    fn existed_min(self, other: Self) -> Self;
    #[must_use]
    fn existed_max(self, other: Self) -> Self;
}

impl<T: Ord> OptionExt for Option<T> {
    fn existed_min(self, other: Self) -> Self {
        match (self, other) {
            (Some(v1), Some(v2)) => Some(v1.min(v2)),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }

    fn existed_max(self, other: Self) -> Self {
        match (self, other) {
            (Some(v1), Some(v2)) => Some(v1.max(v2)),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_min() {
        assert_eq!(Some(1).existed_min(None), Some(1));
        assert_eq!(None.existed_min(Some(1)), Some(1));
        assert_eq!(Some(2).existed_min(Some(3)), Some(2));
    }

    #[test]
    fn t_max() {
        assert_eq!(Some(1).existed_max(None), Some(1));
        assert_eq!(None.existed_max(Some(1)), Some(1));
        assert_eq!(Some(2).existed_max(Some(3)), Some(3));
    }
}
