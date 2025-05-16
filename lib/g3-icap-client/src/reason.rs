/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::fmt;

#[derive(Debug)]
pub enum IcapErrorReason {
    UnknownResponse,
    InvalidResponseAfterContinue,
    UnknownResponseAfterContinue,
    ContinueAfterPreviewEof,
    UnknownResponseForPreview,
    NoBodyFound,
}

impl IcapErrorReason {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            IcapErrorReason::UnknownResponse => "unknown ICAP response code",
            IcapErrorReason::InvalidResponseAfterContinue => {
                "invalid ICAP response code after 100-continue"
            }
            IcapErrorReason::UnknownResponseAfterContinue => {
                "unknown ICAP response code after 100-continue"
            }
            IcapErrorReason::ContinueAfterPreviewEof => {
                "invalid 100-continue response as preview is eof"
            }
            IcapErrorReason::UnknownResponseForPreview => "unknown ICAP response code for preview",
            IcapErrorReason::NoBodyFound => "no ICAP body found",
        }
    }
}

impl fmt::Display for IcapErrorReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
