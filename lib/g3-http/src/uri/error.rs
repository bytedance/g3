/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

#[derive(Debug, Error)]
pub enum UriParseError {
    #[error("required field {0} not found")]
    RequiredFieldNotFound(&'static str),
    #[error("{0} field is not valid scheme")]
    NotValidScheme(&'static str),
    #[error("{0} filed is not valid host string")]
    NotValidHost(&'static str),
    #[error("{0} field is not valid port")]
    NotValidPort(&'static str),
    #[error("{0} field is not valid protocol")]
    NotValidProtocol(&'static str),
}
