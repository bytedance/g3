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

use chrono::format::Numeric::*;
use chrono::format::{Fixed, Item, Pad};

pub const RFC3339_FIXED_MICROSECOND: &[Item<'static>] = &[
    Item::Numeric(Year, Pad::Zero),
    Item::Literal("-"),
    Item::Numeric(Month, Pad::Zero),
    Item::Literal("-"),
    Item::Numeric(Day, Pad::Zero),
    Item::Literal("T"),
    Item::Numeric(Hour, Pad::Zero),
    Item::Literal(":"),
    Item::Numeric(Minute, Pad::Zero),
    Item::Literal(":"),
    Item::Numeric(Second, Pad::Zero),
    Item::Fixed(Fixed::Nanosecond6),
    Item::Fixed(Fixed::TimezoneOffsetColonZ),
];
