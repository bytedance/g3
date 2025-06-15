/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod flex;
pub use flex::FlexBufReader;

mod limited;
pub use limited::LimitedBufReader;

mod once;
pub use once::OnceBufReader;

mod copy;
pub use copy::BufReadCopy;

const DEFAULT_BUF_SIZE: usize = 8 * 1024;
