// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::actor::messages::{
    QueryIndexerEventsMessage, QueryIndexerFieldsMessage, QueryIndexerTransactionsMessage,
    QueryLastStateIndexByTxOrderMessage,
};
use crate::indexer_reader::IndexerReader;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use coerce::actor::{Actor, context::ActorContext, message::Handler};
use kanari_types::indexer::event::IndexerEvent;
use kanari_types::indexer::field::IndexerField;
use kanari_types::indexer::state::IndexerStateID;
use kanari_types::indexer::transaction::IndexerTransaction;
use moveos_types::moveos_std::object::ObjectID;

use super::messages::QueryIndexerObjectIdsMessage;

pub struct IndexerReaderActor {
    indexer_reader: IndexerReader,
}

impl IndexerReaderActor {
    pub fn new(indexer_reader: IndexerReader) -> Result<Self> {
        Ok(Self { indexer_reader })
    }
}

impl Actor for IndexerReaderActor {}

#[async_trait]
impl Handler<QueryIndexerTransactionsMessage> for IndexerReaderActor {
    async fn handle(
        &mut self,
        msg: QueryIndexerTransactionsMessage,
        _ctx: &mut ActorContext,
    ) -> Result<Vec<IndexerTransaction>> {
        let QueryIndexerTransactionsMessage {
            filter,
            cursor,
            limit,
            descending_order,
        } = msg;
        self.indexer_reader
            .query_transactions_with_filter(filter, cursor, limit, descending_order)
            .map_err(|e| anyhow!(format!("Failed to query indexer transactions: {:?}", e)))
    }
}

#[async_trait]
impl Handler<QueryIndexerEventsMessage> for IndexerReaderActor {
    async fn handle(
        &mut self,
        msg: QueryIndexerEventsMessage,
        _ctx: &mut ActorContext,
    ) -> Result<Vec<IndexerEvent>> {
        let QueryIndexerEventsMessage {
            filter,
            cursor,
            limit,
            descending_order,
        } = msg;
        self.indexer_reader
            .query_events_with_filter(filter, cursor, limit, descending_order)
            .map_err(|e| anyhow!(format!("Failed to query indexer events: {:?}", e)))
    }
}

#[async_trait]
impl Handler<QueryIndexerObjectIdsMessage> for IndexerReaderActor {
    async fn handle(
        &mut self,
        msg: QueryIndexerObjectIdsMessage,
        _ctx: &mut ActorContext,
    ) -> Result<Vec<(ObjectID, IndexerStateID)>> {
        let QueryIndexerObjectIdsMessage {
            filter,
            cursor,
            limit,
            descending_order,
            state_type,
        } = msg;
        self.indexer_reader
            .query_object_ids_with_filter(filter, cursor, limit, descending_order, state_type)
            .map_err(|e| anyhow!(format!("Failed to query indexer object states: {:?}", e)))
    }
}

#[async_trait]
impl Handler<QueryLastStateIndexByTxOrderMessage> for IndexerReaderActor {
    async fn handle(
        &mut self,
        msg: QueryLastStateIndexByTxOrderMessage,
        _ctx: &mut ActorContext,
    ) -> Result<Option<u64>> {
        let QueryLastStateIndexByTxOrderMessage {
            tx_order,
            state_type,
        } = msg;

        self.indexer_reader
            .query_last_state_index_by_tx_order(tx_order, state_type)
            .map_err(|e| {
                anyhow!(format!(
                    "Failed to query indexer last state index by tx order: {:?}",
                    e
                ))
            })
    }
}

#[async_trait]
impl Handler<QueryIndexerFieldsMessage> for IndexerReaderActor {
    async fn handle(
        &mut self,
        msg: QueryIndexerFieldsMessage,
        _ctx: &mut ActorContext,
    ) -> Result<Vec<IndexerField>> {
        let QueryIndexerFieldsMessage {
            filter,
            page,
            limit,
            descending_order,
        } = msg;
        self.indexer_reader
            .query_fields_with_filter(filter, page, limit, descending_order)
            .map_err(|e| anyhow!(format!("Failed to query indexer fields: {:?}", e)))
    }
}
