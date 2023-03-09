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

use trust_dns_proto::op::ResponseCode;
use trust_dns_resolver::error::ResolveErrorKind;

use crate::error::{ResolveDriverError, ResolveError, ResolveServerError};

impl From<trust_dns_resolver::error::ResolveError> for ResolveError {
    fn from(e: trust_dns_resolver::error::ResolveError) -> Self {
        match e.kind() {
            ResolveErrorKind::NoRecordsFound { response_code, .. } => match response_code {
                ResponseCode::FormErr => ResolveServerError::FormErr.into(),
                ResponseCode::ServFail => ResolveServerError::ServFail.into(),
                ResponseCode::NXDomain => ResolveServerError::NotFound.into(),
                ResponseCode::NotImp => ResolveServerError::NotImp.into(),
                ResponseCode::Refused => ResolveServerError::Refused.into(),
                ResponseCode::BADNAME => ResolveDriverError::BadName.into(),
                _ => ResolveDriverError::BadResp.into(),
            },
            ResolveErrorKind::Timeout => ResolveDriverError::Timeout.into(),
            _ => ResolveDriverError::Internal(e.to_string()).into(),
        }
    }
}
