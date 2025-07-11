// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::actor::messages::{
    CheckStateChangeSetsMessage, ConvertL2TransactionData, DryRunTransactionResult,
    GetAnnotatedEventsByEventIDsMessage, GetEventsByEventHandleMessage, GetEventsByEventIDsMessage,
    GetStateChangeSetsMessage, GetTxExecutionInfosByHashMessage, ListAnnotatedStatesMessage,
    ListStatesMessage, RefreshStateMessage, SaveStateChangeSetMessage, ValidateL1BlockMessage,
    ValidateL1TxMessage,
};
use crate::actor::reader_executor::ReaderExecutorActor;
use crate::actor::{
    executor::ExecutorActor,
    messages::{
        AnnotatedStatesMessage, ExecuteViewFunctionMessage, GetAnnotatedEventsByEventHandleMessage,
        StatesMessage, ValidateL2TxMessage,
    },
};
use anyhow::{Result, anyhow};
use coerce::actor::ActorRef;
use kanari_types::bitcoin::network::BitcoinNetwork;
use kanari_types::framework::chain_id::ChainID;
use kanari_types::transaction::{
    KanariTransaction, KanariTransactionData, L1BlockWithBody, L1Transaction,
};
use move_core_types::account_address::AccountAddress;
use moveos_types::function_return_value::{AnnotatedFunctionResult, FunctionResult};
use moveos_types::h256::H256;
use moveos_types::module_binding::MoveFunctionCaller;
use moveos_types::moveos_std::account::Account;
use moveos_types::moveos_std::event::{Event, EventID};
use moveos_types::moveos_std::object::{ObjectID, ObjectMeta};
use moveos_types::moveos_std::tx_context::TxContext;
use moveos_types::state::{FieldKey, StateChangeSetExt};
use moveos_types::state_resolver::{AnnotatedStateKV, StateKV};
use moveos_types::transaction::FunctionCall;
use moveos_types::transaction::TransactionExecutionInfo;
use moveos_types::transaction::TransactionOutput;
use moveos_types::{access_path::AccessPath, transaction::VerifiedMoveOSTransaction};
use moveos_types::{
    moveos_std::event::AnnotatedEvent,
    state::{AnnotatedState, ObjectState},
};
use tokio::runtime::Handle;

#[derive(Clone)]
pub struct ExecutorProxy {
    pub actor: ActorRef<ExecutorActor>,
    pub reader_actor: ActorRef<ReaderExecutorActor>,
}

impl ExecutorProxy {
    pub fn new(
        actor: ActorRef<ExecutorActor>,
        reader_actor: ActorRef<ReaderExecutorActor>,
    ) -> Self {
        Self {
            actor,
            reader_actor,
        }
    }

    pub async fn validate_l2_tx(&self, tx: KanariTransaction) -> Result<VerifiedMoveOSTransaction> {
        self.actor.send(ValidateL2TxMessage { tx }).await?
    }

    pub async fn validate_l1_block(
        &self,
        l1_block: L1BlockWithBody,
    ) -> Result<VerifiedMoveOSTransaction> {
        self.actor.send(ValidateL1BlockMessage { l1_block }).await?
    }

    pub async fn validate_l1_tx(
        &self,
        l1_tx: L1Transaction,
        bypass_executed_check: bool,
    ) -> Result<VerifiedMoveOSTransaction> {
        self.actor
            .send(ValidateL1TxMessage {
                l1_tx,
                bypass_executed_check,
            })
            .await?
    }

    pub async fn convert_to_verified_tx(
        &self,
        tx_data: KanariTransactionData,
    ) -> Result<VerifiedMoveOSTransaction> {
        self.actor
            .send(ConvertL2TransactionData { tx_data })
            .await?
    }

    //TODO ensure the execute result
    pub async fn execute_transaction(
        &self,
        tx: VerifiedMoveOSTransaction,
    ) -> Result<(TransactionOutput, TransactionExecutionInfo)> {
        let result = self
            .actor
            .send(crate::actor::messages::ExecuteTransactionMessage { tx })
            .await??;
        Ok((result.output, result.transaction_info))
    }

    pub async fn dry_run_transaction(
        &self,
        tx: VerifiedMoveOSTransaction,
    ) -> Result<DryRunTransactionResult> {
        let result = self
            .actor
            .send(crate::actor::messages::DryRunTransactionMessage { tx })
            .await??;
        Ok(result)
    }

    pub async fn execute_view_function(
        &self,
        call: FunctionCall,
    ) -> Result<AnnotatedFunctionResult> {
        self.reader_actor
            .send(ExecuteViewFunctionMessage { call })
            .await?
    }

    pub async fn get_states(
        &self,
        access_path: AccessPath,
        state_root: Option<H256>,
    ) -> Result<Vec<Option<ObjectState>>> {
        self.reader_actor
            .send(StatesMessage {
                state_root,
                access_path,
            })
            .await?
    }

    pub async fn get_annotated_states(
        &self,
        access_path: AccessPath,
        state_root: Option<H256>,
    ) -> Result<Vec<Option<AnnotatedState>>> {
        self.reader_actor
            .send(AnnotatedStatesMessage {
                state_root,
                access_path,
            })
            .await?
    }

    pub async fn list_states(
        &self,
        state_root: Option<H256>,
        access_path: AccessPath,
        cursor: Option<FieldKey>,
        limit: usize,
    ) -> Result<Vec<StateKV>> {
        self.reader_actor
            .send(ListStatesMessage {
                state_root,
                access_path,
                cursor,
                limit,
            })
            .await?
    }

    pub async fn list_annotated_states(
        &self,
        state_root: Option<H256>,
        access_path: AccessPath,
        cursor: Option<FieldKey>,
        limit: usize,
    ) -> Result<Vec<AnnotatedStateKV>> {
        self.reader_actor
            .send(ListAnnotatedStatesMessage {
                state_root,
                access_path,
                cursor,
                limit,
            })
            .await?
    }

    pub async fn get_annotated_events_by_event_handle(
        &self,
        event_handle_id: ObjectID,
        cursor: Option<u64>,
        limit: u64,
        descending_order: bool,
    ) -> Result<Vec<AnnotatedEvent>> {
        self.reader_actor
            .send(GetAnnotatedEventsByEventHandleMessage {
                event_handle_id,
                cursor,
                limit,
                descending_order,
            })
            .await?
    }

    pub async fn get_events_by_event_handle(
        &self,
        event_handle_id: ObjectID,
        cursor: Option<u64>,
        limit: u64,
        descending_order: bool,
    ) -> Result<Vec<Event>> {
        self.reader_actor
            .send(GetEventsByEventHandleMessage {
                event_handle_id,
                cursor,
                limit,
                descending_order,
            })
            .await?
    }

    pub async fn get_annotated_events_by_event_ids(
        &self,
        event_ids: Vec<EventID>,
    ) -> Result<Vec<Option<AnnotatedEvent>>> {
        self.reader_actor
            .send(GetAnnotatedEventsByEventIDsMessage { event_ids })
            .await?
    }

    pub async fn get_events_by_event_ids(
        &self,
        event_ids: Vec<EventID>,
    ) -> Result<Vec<Option<Event>>> {
        self.reader_actor
            .send(GetEventsByEventIDsMessage { event_ids })
            .await?
    }

    pub async fn get_transaction_execution_infos_by_hash(
        &self,
        tx_hashes: Vec<H256>,
    ) -> Result<Vec<Option<TransactionExecutionInfo>>> {
        self.reader_actor
            .send(GetTxExecutionInfosByHashMessage { tx_hashes })
            .await?
    }

    pub async fn refresh_state(&self, root: ObjectMeta, is_upgrade: bool) -> Result<()> {
        self.reader_actor
            .send(RefreshStateMessage { root, is_upgrade })
            .await?
    }

    /// Get latest root object
    pub async fn get_root(&self) -> Result<ObjectState> {
        self.actor
            .send(crate::actor::messages::GetRootMessage {})
            .await?
    }

    // This is a workaround function to sync the state of the executor to reader
    pub async fn sync_state(&self) -> Result<()> {
        let root = self.get_root().await?;
        self.refresh_state(root.metadata, false).await
    }

    pub async fn save_state_change_set(
        &self,
        tx_order: u64,
        state_change_set: StateChangeSetExt,
    ) -> Result<()> {
        self.actor
            .notify(SaveStateChangeSetMessage {
                tx_order,
                state_change_set,
            })
            .await
            .map_err(|e| anyhow!(format!("Save state change set error: {:?}", e)))
    }

    pub async fn get_state_change_sets(
        &self,
        tx_orders: Vec<u64>,
    ) -> Result<Vec<Option<StateChangeSetExt>>> {
        self.reader_actor
            .send(GetStateChangeSetsMessage { tx_orders })
            .await?
    }

    pub async fn check_state_change_sets(&self, tx_orders: Vec<u64>) -> Result<Vec<u64>> {
        self.reader_actor
            .send(CheckStateChangeSetsMessage { tx_orders })
            .await?
    }

    pub async fn chain_id(&self) -> Result<ChainID> {
        self.get_states(AccessPath::object(ChainID::chain_id_object_id()), None)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("chain id not found"))
            .and_then(|state| state.ok_or_else(|| anyhow::anyhow!("chain id not found")))
            .and_then(|state| Ok(state.into_object::<ChainID>()?.value))
    }

    pub async fn bitcoin_network(&self) -> Result<BitcoinNetwork> {
        self.get_states(AccessPath::object(BitcoinNetwork::object_id()), None)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("bitcoin network not found"))
            .and_then(|state| state.ok_or_else(|| anyhow::anyhow!("bitcoin network not found")))
            .and_then(|state| Ok(state.into_object::<BitcoinNetwork>()?.value))
    }

    //TODO provide a trait to abstract the async state reader, elemiate the duplicated code bwteen RpcService and Client
    pub async fn get_sequence_number(&self, address: AccountAddress) -> Result<u64> {
        Ok(self
            .get_states(
                AccessPath::object(Account::account_object_id(address)),
                None,
            )
            .await?
            .pop()
            .flatten()
            .map(|state| state.into_object::<Account>())
            .transpose()?
            .map_or(0, |account| account.value.sequence_number))
    }
}

impl MoveFunctionCaller for ExecutorProxy {
    fn call_function(
        &self,
        _ctx: &TxContext,
        function_call: FunctionCall,
    ) -> Result<FunctionResult> {
        let executor = self.clone();
        let function_result = tokio::task::block_in_place(|| {
            Handle::current()
                .block_on(async move { executor.execute_view_function(function_call).await })
        })?;
        function_result.try_into()
    }
}
