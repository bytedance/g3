/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use crate::error::{ResolveDriverError, ResolveError, ResolveServerError};

impl ResolveError {
    pub(super) fn from_cares_error(e: c_ares::Error) -> Option<ResolveError> {
        match e {
            c_ares::Error::ENODATA => None, // NODATA is not really an error
            c_ares::Error::EFORMERR => Some(ResolveServerError::FormErr.into()),
            c_ares::Error::ESERVFAIL => Some(ResolveServerError::ServFail.into()),
            c_ares::Error::ENOTFOUND => Some(ResolveServerError::NotFound.into()),
            c_ares::Error::ENOTIMP => Some(ResolveServerError::NotImp.into()),
            c_ares::Error::EREFUSED => Some(ResolveServerError::Refused.into()),
            c_ares::Error::EBADQUERY => Some(ResolveDriverError::BadQuery.into()),
            c_ares::Error::EBADNAME => Some(ResolveDriverError::BadName.into()),
            c_ares::Error::EBADFAMILY => Some(ResolveDriverError::BadFamily.into()),
            c_ares::Error::EBADRESP => Some(ResolveDriverError::BadResp.into()),
            c_ares::Error::ECONNREFUSED => Some(ResolveDriverError::ConnRefused.into()),
            c_ares::Error::ETIMEOUT => Some(ResolveDriverError::Timeout.into()),
            _ => Some(ResolveDriverError::Internal(e.to_string()).into()),
        }
    }
}
