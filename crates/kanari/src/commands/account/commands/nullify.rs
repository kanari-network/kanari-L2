// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use move_core_types::account_address::AccountAddress;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_rpc_api::jsonrpc_types::KanariAddressView;
use kanari_types::address::ParsedAddress;
use kanari_types::{
    address::KanariAddress,
    error::{KanariError, KanariResult},
};
use std::fmt::Debug;

/// Nullify a keypair from a selected coin id with a Kanari address in kanari.keystore
#[derive(Debug, Parser)]
pub struct NullifyCommand {
    #[clap(short = 'a', long = "address", value_parser=ParsedAddress::parse)]
    address: ParsedAddress,
    #[clap(flatten)]
    pub context_options: WalletContextOptions,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<Option<KanariAddressView>> for NullifyCommand {
    async fn execute(self) -> KanariResult<Option<KanariAddressView>> {
        let mut context = self.context_options.build()?;
        let mapping = context.address_mapping();
        let existing_address: KanariAddress =
            self.address.into_kanari_address(&mapping).map_err(|e| {
                KanariError::CommandArgumentError(format!("Invalid Kanari address String: {}", e))
            })?;

        // Remove keypair by coin id from Kanari key store after successfully executing transaction
        context
            .keystore
            .nullify_address(&existing_address)
            .map_err(|e| KanariError::NullifyAccountError(e.to_string()))?;

        if self.json {
            Ok(Some(existing_address.into()))
        } else {
            println!(
                "{}",
                AccountAddress::from(existing_address).to_hex_literal()
            );

            println!("Dropped an existing address {:?}", existing_address,);
            Ok(None)
        }
    }
}
