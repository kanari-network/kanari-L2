// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::non_canonical_clone_impl)]

pub mod api;
pub mod jsonrpc_types;

pub type RpcResult<T> = Result<T, RpcError>;
use jsonrpsee::types::{ErrorCode, ErrorObject, ErrorObjectOwned};
use kanari_types::error::KanariError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error(transparent)]
    KanariError(#[from] KanariError),

    #[error(transparent)]
    InternalError(#[from] anyhow::Error),

    #[error("Deserialization error: {0}")]
    BcsError(#[from] bcs::Error),

    #[error("Unexpected error: {0}")]
    UnexpectedError(String),
}

impl From<RpcError> for ErrorObjectOwned {
    fn from(err: RpcError) -> Self {
        match err {
            RpcError::ServiceUnavailable => ErrorObject::owned(
                ErrorCode::ServerIsBusy.code(),
                "Service unavailable".to_string(),
                None::<()>,
            ),
            RpcError::KanariError(err) => ErrorObject::owned(1, err.to_string(), None::<()>),
            RpcError::InternalError(err) => ErrorObject::owned(2, err.to_string(), None::<()>),
            RpcError::BcsError(err) => ErrorObject::owned(3, err.to_string(), None::<()>),
            RpcError::UnexpectedError(err) => ErrorObject::owned(4, err.to_string(), None::<()>),
        }
    }
}
