// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::commands::db::commands::load_accumulator;
use crate::utils::open_kanari_db;
use accumulator::Accumulator;
use clap::Parser;
use kanari_config::R_OPT_NET_HELP;
use kanari_types::error::KanariResult;
use kanari_types::kanari_network::KanariChainID;
use std::path::PathBuf;

/// Verify Order by Accumulator
#[derive(Debug, Parser)]
pub struct GetAccumulatorLeafByIndexCommand {
    #[clap(long)]
    pub index: u64,
    #[clap(long = "data-dir", short = 'd')]
    pub base_data_dir: Option<PathBuf>,
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,
}

impl GetAccumulatorLeafByIndexCommand {
    pub fn execute(self) -> KanariResult<()> {
        let (_root, kanari_db, _start_time) = open_kanari_db(self.base_data_dir, self.chain_id);
        let kanari_store = kanari_db.kanari_store;
        let (tx_accumulator, _last_tx_order_in_db) = load_accumulator(kanari_store.clone())?;

        let leaf = tx_accumulator.get_leaf(self.index)?;

        println!("{:?}", leaf);
        Ok(())
    }
}
