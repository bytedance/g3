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
