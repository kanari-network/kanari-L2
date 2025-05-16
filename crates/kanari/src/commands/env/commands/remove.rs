// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use kanari_types::error::{KanariError, KanariResult};

use crate::cli_types::WalletContextOptions;

#[derive(Debug, Parser)]
pub struct RemoveCommand {
    #[clap(flatten)]
    pub context_options: WalletContextOptions,
    #[clap(long)]
    env: String,
}

impl RemoveCommand {
    pub async fn execute(self) -> KanariResult<()> {
        let mut context = self.context_options.build()?;
        if let Some(active_env) = &context.client_config.active_env {
            if active_env == &self.env {
                return Err(KanariError::RemoveEnvError(
                    "Cannot remove the currently active environment. Please switch to another environment and try again".to_owned()
                ));
            }
        }

        context
            .client_config
            .envs
            .retain(|env| env.alias != self.env);
        context.client_config.save()?;

        println!("Environment `{}` was successfully removed", self.env);

        Ok(())
    }
}
