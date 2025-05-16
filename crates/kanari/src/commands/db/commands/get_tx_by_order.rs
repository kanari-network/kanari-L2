// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::utils::open_kanari_db;
use clap::Parser;
use kanari_config::R_OPT_NET_HELP;
use kanari_types::error::KanariResult;
use kanari_types::kanari_network::KanariChainID;
use kanari_types::transaction::LedgerTransaction;
use std::path::PathBuf;

/// Get LedgerTransaction by tx_order
#[derive(Debug, Parser)]
pub struct GetTxByOrderCommand {
    #[clap(long)]
    pub order: u64,

    #[clap(long = "data-dir", short = 'd')]
    pub base_data_dir: Option<PathBuf>,
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,
}

impl GetTxByOrderCommand {
    pub fn execute(self) -> KanariResult<Option<LedgerTransaction>> {
        let (_root, kanari_db, _start_time) = open_kanari_db(self.base_data_dir, self.chain_id);
        let kanari_store = kanari_db.kanari_store.clone();

        let tx_opt = kanari_store
            .get_transaction_store()
            .get_tx_by_order(self.order)?;
        Ok(tx_opt)
    }
}
