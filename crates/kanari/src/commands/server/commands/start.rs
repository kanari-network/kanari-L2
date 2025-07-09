// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use kanari_config::{KanariOpt, ServerOpt};
use kanari_key::key_derive::verify_password;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_rpc_server::Service;
use kanari_types::address::KanariAddress;
use kanari_types::error::{KanariError, KanariResult};
use kanari_types::kanari_network::BuiltinChainID;
use rpassword::prompt_password;
use std::str::FromStr;
use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tracing::info;

/// Start service
#[derive(Debug, Parser)]
pub struct StartCommand {
    #[clap(flatten)]
    opt: KanariOpt,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[async_trait]
impl CommandAction<()> for StartCommand {
    async fn execute(mut self) -> KanariResult<()> {
        let mut context = self.context_options.build()?;
        self.opt.init()?;

        //Parse key pair from Kanari opt
        let sequencer_account = if self.opt.sequencer_account.is_none() {
            let active_address_opt = context.client_config.active_address;
            if active_address_opt.is_none() {
                return Err(KanariError::ActiveAddressDoesNotExistError);
            }
            active_address_opt.unwrap()
        } else {
            KanariAddress::from_str(self.opt.sequencer_account.clone().unwrap().as_str()).map_err(
                |e| {
                    KanariError::CommandArgumentError(format!(
                        "Invalid sequencer account address: {}",
                        e
                    ))
                },
            )?
        };
        let proposer_account = if self.opt.proposer_account.is_none() {
            let active_address_opt = context.client_config.active_address;
            if active_address_opt.is_none() {
                return Err(KanariError::ActiveAddressDoesNotExistError);
            }
            active_address_opt.unwrap()
        } else {
            KanariAddress::from_str(self.opt.proposer_account.clone().unwrap().as_str()).map_err(
                |e| {
                    KanariError::CommandArgumentError(format!(
                        "Invalid proposer account address: {}",
                        e
                    ))
                },
            )?
        };

        let (sequencer_keypair, proposer_keypair) = if context.keystore.get_if_password_is_empty() {
            let sequencer_keypair = context
                .keystore
                .get_key_pair(&sequencer_account, None)
                .map_err(|e| KanariError::SequencerKeyPairDoesNotExistError(e.to_string()))?;

            let proposer_keypair = context
                .keystore
                .get_key_pair(&proposer_account, None)
                .map_err(|e| KanariError::ProposerKeyPairDoesNotExistError(e.to_string()))?;

            (sequencer_keypair, proposer_keypair)
        } else {
            let password = prompt_password("Enter the password:").unwrap_or_default();
            let is_verified =
                verify_password(Some(password.clone()), context.keystore.get_password_hash())?;

            if !is_verified {
                return Err(KanariError::InvalidPasswordError(
                    "Password is invalid".to_owned(),
                ));
            }

            let sequencer_keypair = context
                .keystore
                .get_key_pair(&sequencer_account, Some(password.clone()))
                .map_err(|e| KanariError::SequencerKeyPairDoesNotExistError(e.to_string()))?;

            let proposer_keypair = context
                .keystore
                .get_key_pair(&proposer_account, Some(password.clone()))
                .map_err(|e| KanariError::ProposerKeyPairDoesNotExistError(e.to_string()))?;

            (sequencer_keypair, proposer_keypair)
        };
        // Construct sequencer, proposer and relayer keypair
        let mut server_opt = ServerOpt::new();
        server_opt.sequencer_keypair = Some(sequencer_keypair.copy());
        server_opt.proposer_keypair = Some(proposer_keypair.copy());

        let active_env = context.client_config.get_active_env()?;
        server_opt.active_env = Some(active_env.clone().alias);

        let mut service = Service::new();
        service
            .start(self.opt.clone(), server_opt)
            .await
            .map_err(KanariError::from)?;

        // Automatically switch env when use start server, if network is local or dev seed
        let kanari_chain_id = self.opt.chain_id.unwrap_or_default();
        let chain_name = kanari_chain_id.chain_name();
        // When chain_id is not equals to env alias
        let switch_env = if active_env.alias != chain_name {
            if kanari_chain_id.is_local() {
                Some(BuiltinChainID::Local.chain_name())
            } else if kanari_chain_id.is_dev() {
                Some(BuiltinChainID::Dev.chain_name())
            } else {
                println!(
                    "Warning! The active env is not equals to chain_id when server start, current chain_id is `{}`, while active env is `{}`",
                    chain_name, active_env.alias
                );
                None
            }
        } else {
            None
        };

        if let Some(switch_env_alias) = switch_env.clone() {
            if context
                .client_config
                .get_env(&Some(switch_env_alias.clone()))
                .is_none()
            {
                return Err(KanariError::SwitchEnvError(format!(
                    "Auto switch env failed when server start, the env config for `{}` does not exist",
                    switch_env_alias
                )));
            }
            context.client_config.active_env = switch_env;
            context.client_config.save()?;
            println!(
                "The active env was successfully switched to `{}`",
                switch_env_alias
            );
        }

        #[cfg(unix)]
        {
            let mut sig_int = signal(SignalKind::interrupt()).map_err(KanariError::from)?;
            let mut sig_term = signal(SignalKind::terminate()).map_err(KanariError::from)?;
            tokio::select! {
                _ = sig_int.recv() => info!("receive SIGINT"),
                _ = sig_term.recv() => info!("receive SIGTERM"),
                _ = ctrl_c() => info!("receive Ctrl C"),
            }
        }
        #[cfg(not(unix))]
        {
            tokio::select! {
                _ = ctrl_c() => info!("receive Ctrl C"),
            }
        }

        service.stop().map_err(KanariError::from)?;

        info!("Shutdown Sever");
        Ok(())
    }
}
