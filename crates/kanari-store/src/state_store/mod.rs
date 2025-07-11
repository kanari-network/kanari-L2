// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::STATE_CHANGE_SET_COLUMN_FAMILY_NAME;
use anyhow::Result;
use moveos_types::state::StateChangeSetExt;
use raw_store::CodecKVStore;
use raw_store::{StoreInstance, derive_store};

derive_store!(
    StateChangeSetStore,
    u64,
    StateChangeSetExt,
    STATE_CHANGE_SET_COLUMN_FAMILY_NAME
);

pub trait StateStore {
    fn save_state_change_set(
        &self,
        tx_order: u64,
        state_change_set: StateChangeSetExt,
    ) -> Result<()>;
    fn get_state_change_set(&self, tx_order: u64) -> Result<Option<StateChangeSetExt>>;
    fn multi_get_state_change_set(
        &self,
        tx_orders: Vec<u64>,
    ) -> Result<Vec<Option<StateChangeSetExt>>>;
    fn remove_state_change_set(&self, tx_order: u64) -> Result<()>;

    fn check_state_change_set(&self, tx_orders: Vec<u64>) -> Result<Vec<u64>>;
}

#[derive(Clone)]
pub struct StateDBStore {
    state_change_set_store: StateChangeSetStore,
}

impl StateDBStore {
    pub fn new(instance: StoreInstance) -> Self {
        StateDBStore {
            state_change_set_store: StateChangeSetStore::new(instance.clone()),
        }
    }

    pub fn save_state_change_set(
        &self,
        tx_order: u64,
        state_change_set: StateChangeSetExt,
    ) -> Result<()> {
        self.state_change_set_store
            .kv_put(tx_order, state_change_set)
    }

    pub fn get_state_change_set(&self, tx_order: u64) -> Result<Option<StateChangeSetExt>> {
        self.state_change_set_store.kv_get(tx_order)
    }

    pub fn multi_get_state_change_set(
        &self,
        tx_orders: Vec<u64>,
    ) -> Result<Vec<Option<StateChangeSetExt>>> {
        self.state_change_set_store.multiple_get(tx_orders)
    }

    pub fn remove_state_change_set(&self, tx_order: u64) -> Result<()> {
        self.state_change_set_store.remove(tx_order)
    }

    pub fn check_state_change_set(&self, tx_orders: Vec<u64>) -> Result<Vec<u64>> {
        let values = self
            .state_change_set_store
            .multiple_get_raw(tx_orders.clone())?;

        let missing_tx_orders = tx_orders
            .into_iter()
            .zip(values)
            .filter_map(|(k, v)| if v.is_none() { Some(k) } else { None })
            .collect::<Vec<_>>();
        Ok(missing_tx_orders)
    }
}
