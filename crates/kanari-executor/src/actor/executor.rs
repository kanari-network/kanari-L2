// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use super::messages::{
    ConvertL2TransactionData, DryRunTransactionMessage, DryRunTransactionResult,
    ExecuteTransactionMessage, ExecuteTransactionResult, GetRootMessage, SaveStateChangeSetMessage,
    ValidateL1BlockMessage, ValidateL1TxMessage, ValidateL2TxMessage,
};
use crate::metrics::ExecutorMetrics;
use anyhow::Result;
use async_trait::async_trait;
use coerce::actor::{Actor, LocalActorRef, context::ActorContext, message::Handler};
use function_name::named;
use kanari_genesis::FrameworksGasParameters;
use kanari_notify::actor::NotifyActor;
use kanari_notify::event::GasUpgradeEvent;
use kanari_notify::messages::{GasUpgradeMessage, NotifyActorSubscribeMessage};
use kanari_store::KanariStore;
use kanari_store::state_store::StateStore;
use kanari_types::address::{BitcoinAddress, MultiChainAddress};
use kanari_types::bitcoin::BitcoinModule;
use kanari_types::bitcoin::transaction_validator::TransactionValidator as L1TransactionValidator;
use kanari_types::error::KanariError;
use kanari_types::framework::auth_validator::{
    AuthValidatorCaller, BuiltinAuthValidator, TxValidateResult,
};
use kanari_types::framework::ethereum::EthereumModule;
use kanari_types::framework::transaction_validator::TransactionValidator;
use kanari_types::framework::{system_post_execute_functions, system_pre_execute_functions};
use kanari_types::multichain_id::KanariMultiChainID;
use kanari_types::transaction::authenticator::AUTH_PAYLOAD_SIZE;
use kanari_types::transaction::{
    AuthenticatorInfo, KanariTransaction, KanariTransactionData, L1Block, L1BlockWithBody,
    L1Transaction,
};
use move_core_types::account_address::AccountAddress;
use move_core_types::vm_status::VMStatus;
use moveos::moveos::{MoveOS, MoveOSConfig};
use moveos::vm::vm_status_explainer::explain_vm_status;
use moveos_eventbus::bus::EventData;
use moveos_store::MoveOSStore;
use moveos_types::function_return_value::FunctionResult;
use moveos_types::module_binding::MoveFunctionCaller;
use moveos_types::move_std::option::MoveOption;
use moveos_types::moveos_std::object::ObjectMeta;
use moveos_types::moveos_std::tx_context::TxContext;
use moveos_types::moveos_std::tx_meta::TxMeta;
use moveos_types::state::{ObjectState, StateChangeSetExt};
use moveos_types::state_resolver::RootObjectResolver;
use moveos_types::transaction::{FunctionCall, MoveOSTransaction, VerifiedMoveAction};
use moveos_types::transaction::{MoveAction, VerifiedMoveOSTransaction};
use prometheus::Registry;
use std::str::FromStr;
use std::sync::Arc;

pub struct ExecutorActor {
    root: ObjectMeta,
    moveos: MoveOS,
    moveos_store: MoveOSStore,
    kanari_store: KanariStore,
    metrics: Arc<ExecutorMetrics>,
    notify_actor: Option<LocalActorRef<NotifyActor>>,
}

type ValidateAuthenticatorResult = Result<TxValidateResult, VMStatus>;

impl ExecutorActor {
    pub fn new(
        root: ObjectMeta,
        moveos_store: MoveOSStore,
        kanari_store: KanariStore,
        registry: &Registry,
        notify_actor: Option<LocalActorRef<NotifyActor>>,
    ) -> Result<Self> {
        let resolver = RootObjectResolver::new(root.clone(), &moveos_store);
        let gas_parameters = FrameworksGasParameters::load_from_chain(&resolver)?;

        let moveos = MoveOS::new(
            moveos_store.clone(),
            gas_parameters.all_natives(),
            MoveOSConfig::default(),
            system_pre_execute_functions(),
            system_post_execute_functions(),
        )?;

        Ok(Self {
            root,
            moveos,
            moveos_store,
            kanari_store,
            metrics: Arc::new(ExecutorMetrics::new(registry)),
            notify_actor,
        })
    }

    pub async fn subscribe_event(
        &self,
        notify_actor_ref: LocalActorRef<NotifyActor>,
        executor_actor_ref: LocalActorRef<ExecutorActor>,
    ) {
        let gas_upgrade_event = GasUpgradeEvent::default();
        let actor_subscribe_message = NotifyActorSubscribeMessage::new(
            gas_upgrade_event,
            "executor".to_string(),
            Box::new(executor_actor_ref),
        );
        let _ = notify_actor_ref.send(actor_subscribe_message).await;
    }

    pub fn get_kanari_store(&self) -> KanariStore {
        self.kanari_store.clone()
    }

    pub fn get_moveos_store(&self) -> MoveOSStore {
        self.moveos.moveos_store().clone()
    }

    pub fn moveos(&self) -> &MoveOS {
        &self.moveos
    }

    #[named]
    pub fn execute(&mut self, tx: VerifiedMoveOSTransaction) -> Result<ExecuteTransactionResult> {
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .executor_execute_tx_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let tx_hash = tx.ctx.tx_hash();
        let size = tx.ctx.tx_size;
        let (raw_output, _) = self.moveos.execute_only(tx)?;
        let is_gas_upgrade = raw_output.is_gas_upgrade;

        let (output, execution_info) = self.moveos_store.handle_tx_output(tx_hash, raw_output)?;

        self.root = execution_info.root_metadata();
        self.metrics
            .executor_execute_tx_bytes
            .with_label_values(&[fn_name])
            .observe(size as f64);

        if is_gas_upgrade {
            if let Some(notify_actor) = self.notify_actor.clone() {
                let _ = notify_actor.notify(GasUpgradeMessage {});
            }
        }

        Ok(ExecuteTransactionResult {
            output,
            transaction_info: execution_info,
        })
    }

    #[named]
    pub fn dry_run(&mut self, tx: VerifiedMoveOSTransaction) -> Result<DryRunTransactionResult> {
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .executor_execute_tx_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let (raw_output, vm_error_info) = self.moveos.execute_only(tx)?;
        Ok(DryRunTransactionResult {
            raw_output,
            vm_error_info,
        })
    }

    #[named]
    pub fn validate_l1_block(
        &self,
        l1_block: L1BlockWithBody,
    ) -> Result<VerifiedMoveOSTransaction> {
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .executor_validate_tx_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let tx_hash = l1_block.block.tx_hash();
        let tx_size = l1_block.block.tx_size();
        let ctx = TxContext::new_system_call_ctx(tx_hash, tx_size);
        //TODO we should call the contract to validate the l1 block has been executed
        //In the future, we should verify the block PoW difficulty or PoS validator signature before the sequencer decentralized
        let L1BlockWithBody {
            block:
                L1Block {
                    chain_id,
                    block_height,
                    block_hash,
                },
            block_body,
        } = l1_block;
        let result = match KanariMultiChainID::try_from(chain_id.id())? {
            KanariMultiChainID::Bitcoin => {
                let action = VerifiedMoveAction::Function {
                    call: BitcoinModule::create_execute_l1_block_call_bytes(
                        block_height,
                        block_hash,
                        block_body,
                    )?,
                    bypass_visibility: true,
                };
                Ok(VerifiedMoveOSTransaction::new(
                    self.root.clone(),
                    ctx,
                    action,
                ))
            }
            KanariMultiChainID::Ether => {
                let action = VerifiedMoveAction::Function {
                    call: EthereumModule::create_execute_l1_block_call_bytes(block_body),
                    bypass_visibility: true,
                };
                Ok(VerifiedMoveOSTransaction::new(
                    self.root.clone(),
                    ctx,
                    action,
                ))
            }
            id => Err(anyhow::anyhow!("Chain {} not supported yet", id)),
        };

        self.metrics
            .executor_validate_tx_bytes
            .with_label_values(&[fn_name])
            .observe(tx_size as f64);
        result
    }

    #[named]
    pub fn validate_l1_tx(
        &self,
        l1_tx: L1Transaction,
        bypass_executed_check: bool,
    ) -> Result<VerifiedMoveOSTransaction> {
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .executor_validate_tx_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let tx_hash = l1_tx.tx_hash();
        let tx_size = l1_tx.tx_size();
        let result = match KanariMultiChainID::try_from(l1_tx.chain_id.id())? {
            KanariMultiChainID::Bitcoin => {
                // Validate the l1 tx before execution via contract
                if !bypass_executed_check {
                    let readonly_ctx = TxContext::new_readonly_ctx(AccountAddress::ZERO);
                    let l1_tx_validator = self.as_module_binding::<L1TransactionValidator>();
                    let tx_validator_result =
                        l1_tx_validator.validate_l1_tx(&readonly_ctx, tx_hash, vec![])?;
                    // If the l1 tx already execute, skip the tx.
                    if !tx_validator_result {
                        return Err(KanariError::L1TxAlreadyExecuted.into());
                    }
                }

                let action = VerifiedMoveAction::Function {
                    call: BitcoinModule::create_execute_l1_tx_call(l1_tx.block_hash, l1_tx.txid)?,
                    bypass_visibility: true,
                };
                let ctx = TxContext::new_system_call_ctx(tx_hash, tx_size);
                Ok(VerifiedMoveOSTransaction::new(
                    self.root.clone(),
                    ctx,
                    action,
                ))
            }
            id => Err(anyhow::anyhow!("Chain {} not supported yet", id)),
        };

        self.metrics
            .executor_validate_tx_bytes
            .with_label_values(&[fn_name])
            .observe(tx_size as f64);
        result
    }

    #[named]
    pub fn validate_l2_tx(&self, mut tx: KanariTransaction) -> Result<VerifiedMoveOSTransaction> {
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .executor_validate_tx_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let sender = tx.sender();
        let tx_hash = tx.tx_hash();
        tracing::debug!("executor validate_l2_tx: {:?}, sender: {}", tx_hash, sender);

        let authenticator = tx.authenticator_info();
        let mut moveos_tx: MoveOSTransaction = tx.into_moveos_transaction(self.root.clone());
        let tx_size = moveos_tx.ctx.tx_size;
        let tx_result = self.validate_authenticator(&moveos_tx.ctx, authenticator);
        let result = match tx_result {
            Ok(vm_result) => match vm_result {
                Ok(tx_validate_result) => {
                    // Add the tx_validate_result to the context
                    moveos_tx
                        .ctx
                        .add(tx_validate_result)
                        .expect("add tx_validate_result failed");

                    let verify_result = self.moveos.verify(moveos_tx);
                    match verify_result {
                        Ok(verified_tx) => Ok(verified_tx),
                        Err(e) => {
                            tracing::warn!(
                                "transaction verify vm error, tx_hash: {:?}, error:{:?}",
                                tx_hash,
                                e
                            );
                            Err(e.into())
                        }
                    }
                }
                Err(e) => {
                    let resolver = RootObjectResolver::new(self.root.clone(), &self.moveos_store);
                    let status_view = explain_vm_status(&resolver, e.clone())?;
                    tracing::warn!(
                        "transaction validate vm error, tx_hash: {:?}, error:{:?}",
                        tx_hash,
                        status_view,
                    );
                    //TODO how to return the vm status to rpc client.
                    Err(e.into())
                }
            },
            Err(e) => {
                tracing::warn!(
                    "transaction validate error, tx_hash: {:?}, error:{:?}",
                    tx_hash,
                    e
                );
                Err(e)
            }
        };

        self.metrics
            .executor_validate_tx_bytes
            .with_label_values(&[fn_name])
            .observe(tx_size as f64);
        result
    }

    #[named]
    pub fn validate_authenticator(
        &self,
        ctx: &TxContext,
        authenticator: AuthenticatorInfo,
    ) -> Result<ValidateAuthenticatorResult> {
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .executor_validate_tx_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let tx_validator = self.as_module_binding::<TransactionValidator>();
        let tx_validate_function_result = tx_validator
            .validate(ctx, authenticator.clone())?
            .into_result();

        let vm_result = match tx_validate_function_result {
            Ok(tx_validate_result) => {
                let auth_validator_option = tx_validate_result.auth_validator();
                match auth_validator_option {
                    Some(auth_validator) => {
                        let auth_validator_caller = AuthValidatorCaller::new(self, auth_validator);
                        let auth_validator_function_result = auth_validator_caller
                            .validate(ctx, authenticator.authenticator.payload)?
                            .into_result();
                        match auth_validator_function_result {
                            Ok(_) => Ok(tx_validate_result),
                            Err(vm_status) => Err(vm_status),
                        }
                    }
                    None => Ok(tx_validate_result),
                }
            }
            Err(vm_status) => Err(vm_status),
        };

        Ok(vm_result)
    }

    pub fn convert_to_verified_tx_for_dry_run(
        &self,
        tx_data: KanariTransactionData,
    ) -> Result<VerifiedMoveOSTransaction> {
        let root = self.root.clone();

        // The dry run supports unsigned transactions, but when calculating the transaction size,
        // the length of the signature part needs to be included.
        let tx_size = tx_data.tx_size() + AUTH_PAYLOAD_SIZE;

        let mut tx_ctx = TxContext::new(
            tx_data.sender.into(),
            tx_data.sequence_number,
            tx_data.max_gas_amount,
            tx_data.tx_hash(),
            tx_size,
        );

        let tx_metadata = TxMeta::new_from_move_action(&tx_data.action);
        tx_ctx.add(tx_metadata).unwrap();

        let mut bitcoin_address = BitcoinAddress::from_str("18cBEMRxXHqzWWCxZNtU91F5sbUNKhL5PX")?;

        let user_multi_chain_address: MultiChainAddress = tx_data.sender.into();
        if user_multi_chain_address.is_bitcoin_address() {
            bitcoin_address = user_multi_chain_address.try_into()?;
        }

        let dummy_result = TxValidateResult {
            auth_validator_id: BuiltinAuthValidator::Bitcoin.flag().into(),
            auth_validator: MoveOption::none(),
            session_key: MoveOption::none(),
            bitcoin_address,
        };

        tx_ctx.add(dummy_result)?;

        let verified_action = match tx_data.action {
            MoveAction::Script(script_call) => VerifiedMoveAction::Script { call: script_call },
            MoveAction::Function(function_call) => VerifiedMoveAction::Function {
                call: function_call,
                bypass_visibility: false,
            },
            MoveAction::ModuleBundle(module_bundle) => VerifiedMoveAction::ModuleBundle {
                module_bundle,
                init_function_modules: vec![],
            },
        };

        Ok(VerifiedMoveOSTransaction::new(
            root,
            tx_ctx,
            verified_action,
        ))
    }

    pub fn refresh_state(&mut self, root: ObjectMeta, is_upgrade: bool) -> Result<()> {
        self.root = root;
        self.moveos.flush_module_cache(is_upgrade)
    }

    pub fn save_state_change_set(
        &mut self,
        tx_order: u64,
        state_change_set: StateChangeSetExt,
    ) -> Result<()> {
        self.kanari_store
            .save_state_change_set(tx_order, state_change_set)
    }
}

#[async_trait]
impl Actor for ExecutorActor {
    async fn started(&mut self, ctx: &mut ActorContext) {
        let local_actor_ref: LocalActorRef<Self> = ctx.actor_ref();
        if let Some(notify_actor) = self.notify_actor.clone() {
            let _ = self.subscribe_event(notify_actor, local_actor_ref).await;
        }
    }
}

#[async_trait]
impl Handler<ValidateL2TxMessage> for ExecutorActor {
    async fn handle(
        &mut self,
        msg: ValidateL2TxMessage,
        _ctx: &mut ActorContext,
    ) -> Result<VerifiedMoveOSTransaction> {
        self.validate_l2_tx(msg.tx)
    }
}

#[async_trait]
impl Handler<ValidateL1BlockMessage> for ExecutorActor {
    async fn handle(
        &mut self,
        msg: ValidateL1BlockMessage,
        _ctx: &mut ActorContext,
    ) -> Result<VerifiedMoveOSTransaction> {
        self.validate_l1_block(msg.l1_block)
    }
}

#[async_trait]
impl Handler<ValidateL1TxMessage> for ExecutorActor {
    async fn handle(
        &mut self,
        msg: ValidateL1TxMessage,
        _ctx: &mut ActorContext,
    ) -> Result<VerifiedMoveOSTransaction> {
        self.validate_l1_tx(msg.l1_tx, msg.bypass_executed_check)
    }
}

#[async_trait]
impl Handler<ExecuteTransactionMessage> for ExecutorActor {
    async fn handle(
        &mut self,
        msg: ExecuteTransactionMessage,
        _ctx: &mut ActorContext,
    ) -> Result<ExecuteTransactionResult> {
        self.execute(msg.tx)
    }
}

#[async_trait]
impl Handler<GetRootMessage> for ExecutorActor {
    async fn handle(
        &mut self,
        _msg: GetRootMessage,
        _ctx: &mut ActorContext,
    ) -> Result<ObjectState> {
        Ok(ObjectState::new_root(self.root.clone()))
    }
}

#[async_trait]
impl Handler<SaveStateChangeSetMessage> for ExecutorActor {
    async fn handle(
        &mut self,
        msg: SaveStateChangeSetMessage,
        _ctx: &mut ActorContext,
    ) -> Result<()> {
        self.save_state_change_set(msg.tx_order, msg.state_change_set)
    }
}

impl MoveFunctionCaller for ExecutorActor {
    fn call_function(&self, ctx: &TxContext, call: FunctionCall) -> Result<FunctionResult> {
        Ok(self
            .moveos
            .execute_readonly_function(self.root.clone(), ctx, call))
    }
}

#[async_trait]
impl Handler<EventData> for ExecutorActor {
    async fn handle(&mut self, message: EventData, _ctx: &mut ActorContext) -> Result<()> {
        if let Ok(_gas_upgrade_msg) = message.data.downcast::<GasUpgradeEvent>() {
            tracing::info!("ExecutorActor: Reload the MoveOS instance...");

            let resolver = RootObjectResolver::new(self.root.clone(), &self.moveos_store);
            let gas_parameters = FrameworksGasParameters::load_from_chain(&resolver)?;

            self.moveos = MoveOS::new(
                self.moveos_store.clone(),
                gas_parameters.all_natives(),
                MoveOSConfig::default(),
                system_pre_execute_functions(),
                system_post_execute_functions(),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
impl Handler<ConvertL2TransactionData> for ExecutorActor {
    async fn handle(
        &mut self,
        msg: ConvertL2TransactionData,
        _ctx: &mut ActorContext,
    ) -> Result<VerifiedMoveOSTransaction> {
        self.convert_to_verified_tx_for_dry_run(msg.tx_data)
    }
}

#[async_trait]
impl Handler<DryRunTransactionMessage> for ExecutorActor {
    async fn handle(
        &mut self,
        msg: DryRunTransactionMessage,
        _ctx: &mut ActorContext,
    ) -> Result<DryRunTransactionResult> {
        self.dry_run(msg.tx)
    }
}
