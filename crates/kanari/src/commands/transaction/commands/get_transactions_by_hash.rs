// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use moveos_types::h256::H256;
use kanari_rpc_api::jsonrpc_types::transaction_view::TransactionWithInfoView;
use kanari_types::error::KanariResult;

/// Get transactions by hashes
#[derive(Debug, clap::Parser)]
pub struct GetTransactionsByHashCommand {
    /// Transaction's hashes
    #[clap(long, value_delimiter = ',')]
    pub hashes: Vec<H256>,

    #[clap(flatten)]
    pub(crate) context_options: WalletContextOptions,
}

#[async_trait]
impl CommandAction<Vec<Option<TransactionWithInfoView>>> for GetTransactionsByHashCommand {
    async fn execute(self) -> KanariResult<Vec<Option<TransactionWithInfoView>>> {
        let client = self.context_options.build()?.get_client().await?;

        let resp = client.kanari.get_transactions_by_hash(self.hashes).await?;

        Ok(resp)
    }
}
