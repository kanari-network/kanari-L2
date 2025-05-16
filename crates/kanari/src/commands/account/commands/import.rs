// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use fastcrypto::{secp256k1::Secp256k1KeyPair, traits::ToFromBytes};
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_types::{
    crypto::KanariKeyPair,
    error::{KanariError, KanariResult},
    kanari_key::ParsedSecretKey,
};

/// Import an external account from an encoded or a raw material private key into Kanari Key Store.
/// The importing format shall be the same as the exported private keys.
///
/// The command must be specified with -k or --secretkey to import an
/// external account into Kanari Key Store.
#[derive(Debug, Parser)]
pub struct ImportCommand {
    #[clap(short = 'k', long = "secretkey", value_parser=ParsedSecretKey::parse, required = true)]
    secretkey: ParsedSecretKey,
    #[clap(flatten)]
    pub context_options: WalletContextOptions,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<Option<String>> for ImportCommand {
    async fn execute(self) -> KanariResult<Option<String>> {
        let mut context = self.context_options.build_require_password()?;
        let password = context.get_password();
        let kp = KanariKeyPair::Secp256k1(
            Secp256k1KeyPair::from_bytes(&self.secretkey.into_inner().secret_bytes()).map_err(
                |e| KanariError::CommandArgumentError(format!("Invalid Kanari secret key: {}", e)),
            )?,
        );
        let kanari_address = kp.public().kanari_address()?;
        context
            .keystore
            .import_external_account(kanari_address, kp, password)?;

        if self.json {
            Ok(Some(kanari_address.to_string()))
        } else {
            println!(
                "Import succeeded with address [{}] and the secret key",
                kanari_address
            );

            Ok(None)
        }
    }
}
