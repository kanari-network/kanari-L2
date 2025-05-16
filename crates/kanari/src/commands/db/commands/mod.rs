// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use accumulator::MerkleAccumulator;
use raw_store::rocks::RocksDB;
use kanari_config::KanariOpt;
use kanari_store::KanariStore;
use kanari_types::kanari_network::KanariChainID;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::info;

pub mod best_rollback;
pub mod changeset;
pub mod cp_cf;
pub mod drop;
pub mod get_accumulator_leaf_by_index;
pub mod get_changeset_by_order;
pub mod get_execution_info_by_hash;
pub mod get_sequencer_info;
pub mod get_tx_by_order;
pub mod list_anomaly;
pub mod repair;
pub mod revert;
pub mod rollback;
pub mod stat_changeset;
pub mod verify_order;

fn open_rocks(
    base_data_dir: Option<PathBuf>,
    chain_id: Option<KanariChainID>,
) -> anyhow::Result<RocksDB> {
    let opt = KanariOpt::new_with_default(base_data_dir, chain_id, None).unwrap();
    let store_config = opt.store_config();
    let store_dir = store_config.get_store_dir();
    let mut column_families = moveos_store::StoreMeta::get_column_family_names().to_vec();
    column_families.append(&mut kanari_store::StoreMeta::get_column_family_names().to_vec());
    //ensure no duplicate column families
    {
        let mut set = HashSet::with_capacity(column_families.len());
        column_families.iter().for_each(|cf| {
            if !set.insert(cf) {
                panic!("Duplicate column family: {}", cf);
            }
        });
    }

    RocksDB::new(store_dir, column_families, store_config.rocksdb_config())
}

fn load_accumulator(kanari_store: KanariStore) -> anyhow::Result<(MerkleAccumulator, u64)> {
    // The sequencer info would be initialized when genesis, so the sequencer info should not be None
    let last_sequencer_info = kanari_store
        .get_meta_store()
        .get_sequencer_info()?
        .ok_or_else(|| anyhow::anyhow!("Load sequencer info failed"))?;
    let (last_order, last_accumulator_info) = (
        last_sequencer_info.last_order,
        last_sequencer_info.last_accumulator_info.clone(),
    );
    info!("Load latest sequencer order {:?}", last_order);
    info!(
        "Load latest sequencer accumulator info {:?}",
        last_accumulator_info
    );
    let tx_accumulator = MerkleAccumulator::new_with_info(
        last_accumulator_info,
        kanari_store.get_transaction_accumulator_store(),
    );
    Ok((tx_accumulator, last_order))
}
