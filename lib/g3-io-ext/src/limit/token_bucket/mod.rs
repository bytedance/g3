/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod stream;
pub use stream::GlobalStreamLimiter;

mod datagram;
pub use datagram::GlobalDatagramLimiter;
