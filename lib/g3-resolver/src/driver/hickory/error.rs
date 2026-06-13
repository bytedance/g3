/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use hickory_net::{DnsError, NetError};
use hickory_proto::ProtoError;
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
        ResolveDriverError::Internal(value.to_string())
    }
}
impl From<DnsError> for ResolveError {
    fn from(value: DnsError) -> Self {
        match value {
            DnsError::ResponseCode(c) => {
                ResolveError::from_response_code(c).unwrap_or(ResolveError::EmptyResult)
            }
            DnsError::NoRecordsFound(c) => ResolveError::from_response_code(c.response_code)
                .unwrap_or(ResolveError::EmptyResult),
            _ => ResolveError::UnexpectedError("unexpected DNS error"),
        }
    }
}

impl From<NetError> for ResolveError {
    fn from(value: NetError) -> Self {
        match value {
            NetError::Dns(e) => ResolveError::from(e),
            NetError::Timeout => ResolveDriverError::Timeout.into(),
            NetError::Proto(e) => ResolveDriverError::from(&e).into(),
            e => ResolveDriverError::Internal(e.to_string()).into(),
        }
    }
}
