// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;
use clap::Parser;
use kanari_config::R_OPT_NET_HELP;
use kanari_types::error::{KanariError, KanariResult};
use kanari_types::kanari_network::KanariChainID;
use std::path::PathBuf;

use crate::utils::open_kanari_db;

/// Revert tx by db command.
#[derive(Debug, Parser)]
pub struct RevertCommand {
    #[clap(long, short = 'o')]
    pub tx_order: u64,

    #[clap(long = "data-dir", short = 'd')]
    pub base_data_dir: Option<PathBuf>,
    #[clap(long, short = 'n', help = R_OPT_NET_HELP)]
    pub chain_id: Option<KanariChainID>,
}

impl RevertCommand {
    pub async fn execute(self) -> KanariResult<()> {
        let tx_order = self.tx_order;
        if tx_order == 0 {
            return Err(KanariError::from(Error::msg(
                "tx order should be greater than 0",
            )));
        }
        let (_root, kanari_db, _start_time) = open_kanari_db(self.base_data_dir, self.chain_id);

        let tx_hashes = kanari_db
            .kanari_store
            .transaction_store
            .get_tx_hashes(vec![tx_order])?;
        // check tx hash exist via tx_order
        if tx_hashes.is_empty() || tx_hashes[0].is_none() {
            return Err(KanariError::from(Error::msg(format!(
                "revert tx failed: tx_hash not found for tx_order {:?}",
                tx_order
            ))));
        }
        let tx_hash = tx_hashes[0].unwrap();

        kanari_db.revert_tx(tx_hash)?;

        Ok(())
    }
}
