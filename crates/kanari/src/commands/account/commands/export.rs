// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_rpc_api::jsonrpc_types::export_view::ExportInfoView;
use kanari_types::{
    address::{KanariAddress, ParsedAddress},
    error::{KanariError, KanariResult},
    kanari_key::KANARI_SECRET_KEY_HRP,
};

/// Export an existing private key for one address or mnemonic for all addresses off-chain.
///
/// Default to export all addresses with a mnemonic phrase but can be specified with -a or
/// --address to export only one address with a private key.
#[derive(Debug, Parser)]
pub struct ExportCommand {
    #[clap(short = 'a', long = "address", value_parser=ParsedAddress::parse, default_value = "")]
    address: ParsedAddress,
    #[clap(flatten)]
    pub context_options: WalletContextOptions,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<Option<ExportInfoView>> for ExportCommand {
    async fn execute(self) -> KanariResult<Option<ExportInfoView>> {
        let mut context = self.context_options.build_require_password()?;
        let password = context.get_password();
        let result = if self.address == ParsedAddress::Named("".to_owned()) {
            context.keystore.export_mnemonic_phrase(password)?
        } else {
            let mapping = context.address_mapping();
            let kanari_address: KanariAddress =
                self.address.into_kanari_address(&mapping).map_err(|e| {
                    KanariError::CommandArgumentError(format!(
                        "Invalid Kanari address String: {}",
                        e
                    ))
                })?;
            let kp = context.keystore.get_key_pair(&kanari_address, password)?;
            kp.export_private_key().map_err(|e| {
                KanariError::CommandArgumentError(format!(
                    "Failed to export private key due to the encoding error of the key: {}",
                    e
                ))
            })?
        };

        if self.json {
            if result.starts_with(KANARI_SECRET_KEY_HRP.as_str()) {
                Ok(Some(ExportInfoView::new_encoded_private_key(result)))
            } else {
                Ok(Some(ExportInfoView::new_mnemonic_phrase(result)))
            }
        } else {
            if result.starts_with(KANARI_SECRET_KEY_HRP.as_str()) {
                println!("Export succeeded with the encoded private key [{}]", result);
            } else {
                println!("Export succeeded with the mnemonic phrase [{}]", result);
            };

            Ok(None)
        }
    }
}
