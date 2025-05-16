// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, FileOrHexInput, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use kanari_types::error::KanariResult;

#[derive(Debug, Parser)]
pub struct BroadcastTx {
    /// The input tx file path or hex string
    input: FileOrHexInput,
    #[clap(flatten)]
    pub(crate) context_options: WalletContextOptions,
}

#[async_trait]
impl CommandAction<String> for BroadcastTx {
    async fn execute(self) -> KanariResult<String> {
        let context = self.context_options.build()?;
        let client = context.get_client().await?;

        Ok(client
            .kanari
            .broadcast_bitcoin_tx(&self.input.data, None, None)
            .await?)
    }
}
