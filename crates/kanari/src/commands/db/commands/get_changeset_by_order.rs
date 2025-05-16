// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::utils::open_kanari_db;
use clap::Parser;
use moveos_types::state::StateChangeSetExt;
use kanari_config::R_OPT_NET_HELP;
use kanari_store::state_store::StateStore;
use kanari_types::error::KanariResult;
use kanari_types::kanari_network::KanariChainID;
use std::path::PathBuf;

/// Get changeset by order
#[derive(Debug, Parser)]
pub struct GetChangesetByOrderCommand {
    #[clap(long)]
    pub order: u64,

    #[clap(long = "data-dir", short = 'd')]
    pub base_data_dir: Option<PathBuf>,
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,
}

impl GetChangesetByOrderCommand {
    pub async fn execute(self) -> KanariResult<Option<StateChangeSetExt>> {
        let (_root, kanari_db, _start_time) = open_kanari_db(self.base_data_dir, self.chain_id);
        let kanari_store = kanari_db.kanari_store;
        let tx_order = self.order;
        let state_change_set_ext_opt = kanari_store.get_state_change_set(tx_order)?;
        Ok(state_change_set_ext_opt)
    }
}
