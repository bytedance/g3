/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use super::{HeaderBufOffsets, HeaderTransportResponse};

mod builder;
pub(crate) use builder::KitexTTHeaderBuilder;

mod reader;
pub(crate) use reader::KitexTTHeaderReader;
