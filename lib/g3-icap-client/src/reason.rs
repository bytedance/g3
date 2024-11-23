/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::fmt;

#[derive(Debug)]
pub(crate) enum IcapErrorReason {
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
