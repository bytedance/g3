/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use chrono::format::Numeric::*;
use chrono::format::{Fixed, Item, Pad};

pub const RFC3659: &[Item<'static>] = &[
    Item::Numeric(Year, Pad::Zero),
    Item::Numeric(Month, Pad::Zero),
    Item::Numeric(Day, Pad::Zero),
    Item::Numeric(Hour, Pad::Zero),
    Item::Numeric(Minute, Pad::Zero),
    Item::Numeric(Second, Pad::Zero),
    Item::Fixed(Fixed::Nanosecond),
];
