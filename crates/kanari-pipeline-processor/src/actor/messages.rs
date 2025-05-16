// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use coerce::actor::message::Message;
use kanari_types::{
    service_status::ServiceStatus,
    transaction::{ExecuteTransactionResponse, L1BlockWithBody, L1Transaction, KanariTransaction},
};

#[derive(Clone)]
pub struct ExecuteL2TxMessage {
    pub tx: KanariTransaction,
}

impl Message for ExecuteL2TxMessage {
    type Result = Result<ExecuteTransactionResponse>;
}

#[derive(Clone)]
pub struct ExecuteL1BlockMessage {
    pub tx: L1BlockWithBody,
}

impl Message for ExecuteL1BlockMessage {
    type Result = Result<ExecuteTransactionResponse>;
}

#[derive(Clone)]
pub struct ExecuteL1TxMessage {
    pub tx: L1Transaction,
}

impl Message for ExecuteL1TxMessage {
    type Result = Result<ExecuteTransactionResponse>;
}

#[derive(Clone)]
pub struct GetServiceStatusMessage {}

impl Message for GetServiceStatusMessage {
    type Result = Result<ServiceStatus>;
}
