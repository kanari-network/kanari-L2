// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::address::{KanariAddress, KanariSupportedAddress};
use crate::transaction::authenticator::Authenticator;
use crate::transaction::kanari::{KanariTransaction, KanariTransactionData};
use crate::transaction::{LedgerTransaction, TransactionSequenceInfo};
use accumulator::accumulator_info::AccumulatorInfo;
use ethers::types::H256;
use move_core_types::account_address::AccountAddress;
use moveos_types::moveos_std::object::ObjectID;
use moveos_types::state::FieldKey;
use rand::{Rng, thread_rng};

use crate::crypto::KanariKeyPair;
use crate::indexer::field::IndexerField;
use crate::indexer::state::IndexerObjectState;
pub use moveos_types::test_utils::*;

pub fn random_kanari_transaction() -> KanariTransaction {
    let move_action_type = random_move_action_type();
    random_kanari_transaction_with_move_action(move_action_type)
}

pub fn random_ledger_transaction() -> LedgerTransaction {
    let kanari_transaction = random_kanari_transaction();

    let tx_order_signature = random_bytes();
    let accumulator_info = random_accumulator_info();
    let random_sequence_info =
        TransactionSequenceInfo::new(rand::random(), tx_order_signature, accumulator_info, 0);
    LedgerTransaction::new_l2_tx(kanari_transaction, random_sequence_info)
}

pub fn random_ledger_transaction_with_order(
    tx_order: u64,
    keypair: &KanariKeyPair,
) -> LedgerTransaction {
    let mut kanari_transaction = random_kanari_transaction();
    let tx_hash = kanari_transaction.tx_hash();
    let tx_order_signature = LedgerTransaction::sign_tx_order(tx_order, tx_hash, keypair);
    let accumulator_info = random_accumulator_info();
    let random_sequence_info =
        TransactionSequenceInfo::new(tx_order, tx_order_signature, accumulator_info, 0);
    LedgerTransaction::new_l2_tx(kanari_transaction, random_sequence_info)
}

pub fn random_kanari_transaction_with_move_action(
    move_action: MoveActionType,
) -> KanariTransaction {
    let mut rng = thread_rng();
    let sequence_number = rng.gen_range(1..=100);
    let tx_data = KanariTransactionData::new_for_test(
        KanariAddress::random(),
        sequence_number,
        random_move_action_with_action_type(move_action.action_type()),
    );

    let mut rng = thread_rng();
    let auth_validator_id = rng.gen_range(1..=100);
    let authenticator = Authenticator::new(auth_validator_id, random_bytes());

    KanariTransaction::new(tx_data, authenticator)
}

pub fn random_accumulator_info() -> AccumulatorInfo {
    let mut rng = thread_rng();
    let num_leaves = rng.gen_range(1..=100) as u64;
    let num_nodes = rng.gen_range(1..=100) as u64;
    AccumulatorInfo::new(H256::random(), vec![], num_leaves, num_nodes)
}

pub fn random_new_object_states() -> Vec<IndexerObjectState> {
    // new_object_states
    let mut rng = thread_rng();
    random_new_object_states_with_size(rng.gen_range(1..=10))
}

pub fn random_new_object_states_with_size(size: usize) -> Vec<IndexerObjectState> {
    let mut new_object_states = vec![];

    for (state_index, _n) in (0..size).enumerate() {
        let state = IndexerObjectState::new(
            random_table_object().into_state().metadata,
            size as u64,
            state_index as u64,
        );

        new_object_states.push(state);
    }

    new_object_states
}

pub fn random_new_object_states_with_size_and_tx_order(
    size: usize,
    tx_order: u64,
) -> Vec<IndexerObjectState> {
    let mut new_object_states = vec![];

    for (state_index, _n) in (0..size).enumerate() {
        let state = IndexerObjectState::new(
            random_table_object().into_state().metadata,
            tx_order,
            state_index as u64,
        );

        new_object_states.push(state);
    }

    new_object_states
}

pub fn random_update_object_states(states: Vec<IndexerObjectState>) -> Vec<IndexerObjectState> {
    states
        .into_iter()
        .map(|item| {
            let mut metadata = item.metadata;
            metadata.size += 1;
            metadata.updated_at += 1;

            IndexerObjectState {
                metadata,
                tx_order: item.tx_order,
                state_index: item.state_index,
            }
        })
        .collect()
}

pub fn random_remove_object_states() -> Vec<String> {
    let mut remove_object_states = vec![];

    let mut rng = thread_rng();
    for _n in 0..rng.gen_range(1..=10) {
        let object_id = ObjectID::from(AccountAddress::random());
        remove_object_states.push(object_id.to_string());
    }

    remove_object_states
}

pub fn random_new_fields() -> Vec<IndexerField> {
    let mut rng = thread_rng();
    random_new_fields_with_size(rng.gen_range(1..=10))
}

pub fn random_new_fields_with_size(size: usize) -> Vec<IndexerField> {
    let mut new_fields = vec![];
    let mut rng = thread_rng();

    for _n in 0..size {
        let sort_key = rng.gen_range(1..=100000);
        let field = IndexerField::new(
            random_table_object().into_state().metadata,
            FieldKey::random(),
            sort_key as u64,
        );

        new_fields.push(field);
    }

    new_fields
}

pub fn random_update_fields(fields: Vec<IndexerField>) -> Vec<IndexerField> {
    fields
        .into_iter()
        .map(|mut item| {
            item.metadata.updated_at += 1;
            IndexerField {
                field_key: item.field_key,
                metadata: item.metadata,
                sort_key: item.sort_key + 1,
            }
        })
        .collect()
}

pub fn random_remove_fields() -> Vec<String> {
    let mut remove_fields = vec![];

    let mut rng = thread_rng();
    for _n in 0..rng.gen_range(1..=10) {
        let object_id = ObjectID::from(AccountAddress::random());
        remove_fields.push(object_id.to_string());
    }

    remove_fields
}

pub fn random_remove_fields_by_parent_id() -> Vec<String> {
    let mut remove_fields = vec![];

    let mut rng = thread_rng();
    for _n in 0..rng.gen_range(1..=10) {
        let object_id = ObjectID::from(AccountAddress::random());
        remove_fields.push(object_id.to_string());
    }

    remove_fields
}
