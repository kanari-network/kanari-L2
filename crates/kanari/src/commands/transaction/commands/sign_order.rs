// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::WalletContextOptions;
use crate::utils::get_sequencer_keypair;
use kanari_types::error::KanariResult;
use kanari_types::transaction::LedgerTransaction;
use moveos_types::h256::H256;

/// Get transactions by hashes
#[derive(Debug, clap::Parser)]
pub struct SignOrderCommand {
    /// Transaction's hash
    #[clap(long)]
    pub tx_hash: H256,
    #[clap(long)]
    pub tx_order: u64,
    #[clap(long)]
    pub sequencer_account: Option<String>,
    #[clap(flatten)]
    pub(crate) context_options: WalletContextOptions,
}

impl SignOrderCommand {
    pub fn execute(self) -> KanariResult<String> {
        let sequencer_keypair =
            get_sequencer_keypair(self.context_options, self.sequencer_account)?;
        let tx_order_sign =
            LedgerTransaction::sign_tx_order(self.tx_order, self.tx_hash, &sequencer_keypair);
        let tx_order_sign_str = serde_json::to_string(&tx_order_sign)?;
        Ok(tx_order_sign_str)
    }
}
