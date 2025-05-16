// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::utils::open_kanari_db;
use clap::Parser;
use kanari_config::R_OPT_NET_HELP;
use kanari_store::meta_store::MetaStore;
use kanari_types::error::KanariResult;
use kanari_types::kanari_network::KanariChainID;
use kanari_types::sequencer::SequencerInfo;
use std::path::PathBuf;

/// Get ExecutionInfo by tx_hash
#[derive(Debug, Parser)]
pub struct GetSequencerInfoCommand {
    #[clap(long = "data-dir", short = 'd')]
    pub base_data_dir: Option<PathBuf>,
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,
}

impl GetSequencerInfoCommand {
    pub fn execute(self) -> KanariResult<Option<SequencerInfo>> {
        let (_root, kanari_db, _start_time) = open_kanari_db(self.base_data_dir, self.chain_id);
        let moveos_store = kanari_db.kanari_store.clone();

        let sequencer_info = moveos_store.get_sequencer_info()?;
        Ok(sequencer_info)
    }
}
