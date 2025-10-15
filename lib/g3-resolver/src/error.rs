/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ResolveServerError {
    #[error("server claims query was malformed")]
    FormErr,
    #[error("server returned general failure")]
    ServFail,
    #[error("server claims domain name not found")]
    NotFound,
    #[error("server does not implement requested operation")]
    NotImp,
    #[error("server refused query")]
    Refused,
}

impl ResolveServerError {
    pub fn get_type(&self) -> &str {
        match self {
            ResolveServerError::FormErr => "FORMERR",
            ResolveServerError::ServFail => "SERVFAIL",
            ResolveServerError::NotFound => "NOTFOUND",
            ResolveServerError::NotImp => "NOTIMP",
            ResolveServerError::Refused => "REFUSED",
        }
    }
}

#[derive(Error, Debug, Clone)]
pub enum ResolveDriverError {
    #[error("malformed DNS query")]
    BadQuery,
    #[error("malformed domain name")]
    BadName,
    #[error("unsupported address family")]
    BadFamily,
    #[error("malformed DNS reply")]
    BadResp,
    #[error("connection refused by server")]
    ConnRefused,
    #[error("timeout while contacting server")]
    Timeout,
    #[error("internal error: {0}")]
    Internal(String),
}

impl ResolveDriverError {
    pub fn get_type(&self) -> &str {
        match self {
            ResolveDriverError::BadQuery => "BadQuery",
            ResolveDriverError::BadName => "BadName",
            ResolveDriverError::BadFamily => "BadFamily",
            ResolveDriverError::BadResp => "BadResp",
            ResolveDriverError::ConnRefused => "ConnRefused",
            ResolveDriverError::Timeout => "Timeout",
            ResolveDriverError::Internal(_) => "InternalError",
        }
    }
}

#[derive(Error, Debug, Clone)]
pub enum ResolveLocalError {
    #[error("no resolver set")]
    NoResolverSet,
    #[error("no resolver running")]
    NoResolverRunning,
    #[error("driver timed out")]
    DriverTimedOut,
}

impl ResolveLocalError {
    pub fn get_type(&self) -> &str {
        match self {
            ResolveLocalError::NoResolverSet => "NoResolverSet",
            ResolveLocalError::NoResolverRunning => "NoResolverRunning",
            ResolveLocalError::DriverTimedOut => "DriverTimedOut",
        }
    }
}

#[derive(Error, Debug, Clone)]
pub enum ResolveError {
    #[error("empty domain")]
    EmptyDomain,
    #[error("empty result")]
    EmptyResult,
    #[error("server error: {0}")]
    FromServer(#[from] ResolveServerError),
    #[error("driver error: {0}")]
    FromDriver(#[from] ResolveDriverError),
    #[error("local error: {0}")]
    FromLocal(#[from] ResolveLocalError),
    #[error("unexpected error: {0}")]
    UnexpectedError(&'static str),
}

impl ResolveError {
    pub fn get_type(&self) -> &str {
        match self {
            ResolveError::EmptyDomain => "EmptyDomain",
            ResolveError::EmptyResult => "EmptyResult",
            ResolveError::FromServer(_) => "ServerError",
            ResolveError::FromDriver(_) => "DriverError",
            ResolveError::FromLocal(_) => "LocalError",
            ResolveError::UnexpectedError(_) => "UnexpectedError",
        }
    }

    pub fn get_subtype(&self) -> &str {
        match self {
            ResolveError::EmptyDomain | ResolveError::EmptyResult => "",
            ResolveError::FromServer(e) => e.get_type(),
            ResolveError::FromDriver(e) => e.get_type(),
            ResolveError::FromLocal(e) => e.get_type(),
            ResolveError::UnexpectedError(_) => "",
        }
    }
}
