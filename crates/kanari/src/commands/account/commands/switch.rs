// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_rpc_api::jsonrpc_types::KanariAddressView;
use kanari_types::address::ParsedAddress;
use kanari_types::{
    address::KanariAddress,
    error::{KanariError, KanariResult},
};
use std::fmt::Debug;

/// Switch the active Kanari account
#[derive(Debug, Parser)]
pub struct SwitchCommand {
    #[clap(flatten)]
    pub context_options: WalletContextOptions,
    /// The address of the Kanari account to be set as active
    #[clap(short = 'a', long = "address", value_parser=ParsedAddress::parse)]
    address: ParsedAddress,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<Option<KanariAddressView>> for SwitchCommand {
    async fn execute(self) -> KanariResult<Option<KanariAddressView>> {
        let mut context = self.context_options.build()?;
        let mapping = context.address_mapping();
        let kanari_address: KanariAddress =
            self.address.into_kanari_address(&mapping).map_err(|e| {
                KanariError::CommandArgumentError(format!("Invalid Kanari address String: {}", e))
            })?;

        if !context.keystore.addresses().contains(&kanari_address) {
            return Err(KanariError::SwitchAccountError(format!(
                "Address `{}` does not in the Kanari keystore",
                kanari_address
            )));
        }

        context.client_config.active_address = Some(kanari_address);
        context.client_config.save()?;

        if self.json {
            Ok(Some(kanari_address.into()))
        } else {
            println!(
                "The active account was successfully switched to `{}`",
                kanari_address
            );

            Ok(None)
        }
    }
}
