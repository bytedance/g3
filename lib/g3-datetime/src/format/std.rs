/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
