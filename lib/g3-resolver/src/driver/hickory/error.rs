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

use hickory_client::error::{ClientError, ClientErrorKind};
use hickory_proto::error::{DnsSecError, DnsSecErrorKind, ProtoError, ProtoErrorKind};
use hickory_proto::op::ResponseCode;

use crate::error::{ResolveDriverError, ResolveError, ResolveServerError};

impl ResolveError {
    pub(super) fn from_response_code(code: ResponseCode) -> Option<Self> {
        match code {
            ResponseCode::NoError => None,
            ResponseCode::FormErr => Some(ResolveServerError::FormErr.into()),
            ResponseCode::ServFail => Some(ResolveServerError::ServFail.into()),
            ResponseCode::NXDomain => Some(ResolveServerError::NotFound.into()),
            ResponseCode::NotImp => Some(ResolveServerError::NotImp.into()),
            ResponseCode::Refused => Some(ResolveServerError::Refused.into()),
            ResponseCode::BADNAME => Some(ResolveDriverError::BadName.into()),
            _ => Some(ResolveDriverError::BadResp.into()),
        }
    }
}

impl From<&ProtoError> for ResolveDriverError {
    fn from(value: &ProtoError) -> Self {
        match value.kind() {
            ProtoErrorKind::Timeout => ResolveDriverError::Timeout,
            ProtoErrorKind::DomainNameTooLong(_) => ResolveDriverError::BadName,
            _ => ResolveDriverError::Internal(value.to_string()),
        }
    }
}

impl From<&DnsSecError> for ResolveDriverError {
    fn from(value: &DnsSecError) -> Self {
        match value.kind() {
            DnsSecErrorKind::Timeout => ResolveDriverError::Timeout,
            DnsSecErrorKind::Proto(e) => ResolveDriverError::from(e),
            _ => ResolveDriverError::Internal(value.to_string()),
        }
    }
}

impl From<ClientError> for ResolveError {
    fn from(value: ClientError) -> Self {
        let driver_error = match value.kind() {
            ClientErrorKind::Timeout => ResolveDriverError::Timeout,
            ClientErrorKind::Proto(e) => ResolveDriverError::from(e),
            ClientErrorKind::DnsSec(e) => ResolveDriverError::from(e),
            _ => ResolveDriverError::Internal(value.to_string()),
        };
        ResolveError::FromDriver(driver_error)
    }
}
