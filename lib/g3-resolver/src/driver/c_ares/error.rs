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
