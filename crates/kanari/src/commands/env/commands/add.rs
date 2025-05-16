// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::WalletContextOptions;
use clap::{Parser, ValueHint};
use kanari_rpc_client::client_config::Env;
use kanari_types::error::KanariResult;
use std::time::Duration;

/// Add a new Kanari environment
#[derive(Debug, Parser)]
pub struct AddCommand {
    #[clap(flatten)]
    pub context_options: WalletContextOptions,
    #[clap(long)]
    pub alias: String,
    #[clap(long, value_hint = ValueHint::Url)]
    pub rpc: String,
    #[clap(long, value_hint = ValueHint::Url)]
    pub ws: Option<String>,
}

impl AddCommand {
    pub async fn execute(self) -> KanariResult<()> {
        let mut context = self.context_options.build()?;
        let AddCommand { alias, rpc, ws, .. } = self;
        let env = Env {
            ws,
            rpc,
            alias: alias.clone(),
        };

        // TODO: is this request timeout okay?
        env.create_rpc_client(Duration::from_secs(5)).await?;
        context.client_config.add_env(env);
        context.client_config.save()?;

        println!("Environment `{} was successfully added", alias);

        Ok(())
    }
}
