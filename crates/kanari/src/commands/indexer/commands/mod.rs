// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use anyhow::Result;

use kanari_config::KanariOpt;
use kanari_indexer::IndexerStore;
use kanari_indexer::indexer_reader::IndexerReader;
use kanari_types::kanari_network::KanariChainID;
use metrics::RegistryService;

pub mod bench;
pub mod rebuild;

pub const BATCH_SIZE: usize = 5000;
fn init_indexer(
    base_data_dir: Option<PathBuf>,
    chain_id: Option<KanariChainID>,
) -> Result<(IndexerStore, IndexerReader)> {
    // Reconstruct KanariOpt
    let opt = KanariOpt::new_with_default(base_data_dir, chain_id, None)?;

    let store_config = opt.store_config();
    let registry_service = RegistryService::default();

    let indexer_db_path = store_config.get_indexer_dir();
    let indexer_store = IndexerStore::new(
        indexer_db_path.clone(),
        &registry_service.default_registry(),
    )?;
    let indexer_reader = IndexerReader::new(indexer_db_path, &registry_service.default_registry())?;

    Ok((indexer_store, indexer_reader))
}
