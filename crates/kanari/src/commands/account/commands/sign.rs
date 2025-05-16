// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use moveos_types::state::MoveState;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_types::{
    address::ParsedAddress,
    error::KanariResult,
    framework::auth_payload::{SignData, MESSAGE_INFO_PREFIX},
};

/// Sign a message with a parsed address
#[derive(Debug, Parser)]
pub struct SignCommand {
    // An address to be used
    #[clap(short = 'a', long = "address", value_parser=ParsedAddress::parse, default_value = "")]
    address: ParsedAddress,

    /// A message to be signed
    #[clap(short = 'm', long)]
    message: String,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<Option<String>> for SignCommand {
    async fn execute(self) -> KanariResult<Option<String>> {
        let context = self.context_options.build_require_password()?;
        let password = context.get_password();
        let mapping = context.address_mapping();
        let kanari_address = self.address.into_kanari_address(&mapping)?;

        let sign_data =
            SignData::new_without_tx_hash(MESSAGE_INFO_PREFIX.to_vec(), self.message.to_bytes());
        let encoded_sign_data = sign_data.encode();

        let signature =
            context
                .keystore
                .sign_hashed(&kanari_address, &encoded_sign_data, password)?;

        let signature_bytes = signature.as_ref();
        let signature_hex = hex::encode(signature_bytes);

        if self.json {
            Ok(Some(signature_hex))
        } else {
            println!(
                "Sign message succeeded with the signatue {:?}",
                signature_hex
            );
            Ok(None)
        }
    }
}
