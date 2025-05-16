// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::utils::open_kanari_db;
use clap::Parser;
use moveos_store::transaction_store::TransactionStore;
use moveos_types::h256::H256;
use moveos_types::transaction::TransactionExecutionInfo;
use kanari_config::R_OPT_NET_HELP;
use kanari_types::error::KanariResult;
use kanari_types::kanari_network::KanariChainID;
use std::path::PathBuf;

/// Get ExecutionInfo by tx_hash
#[derive(Debug, Parser)]
pub struct GetExecutionInfoByHashCommand {
    #[clap(long)]
    pub hash: H256,

    #[clap(long = "data-dir", short = 'd')]
    pub base_data_dir: Option<PathBuf>,
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,
}

impl GetExecutionInfoByHashCommand {
    pub fn execute(self) -> KanariResult<Option<TransactionExecutionInfo>> {
        let (_root, kanari_db, _start_time) = open_kanari_db(self.base_data_dir, self.chain_id);
        let moveos_store = kanari_db.moveos_store.clone();

        let execution_info = moveos_store
            .get_transaction_store()
            .get_tx_execution_info(self.hash)?;
        Ok(execution_info)
    }
}
