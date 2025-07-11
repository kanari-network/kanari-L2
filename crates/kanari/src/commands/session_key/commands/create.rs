// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{TransactionOptions, WalletContextOptions};
use clap::Parser;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_types::{
    address::KanariAddress,
    error::{KanariError, KanariResult},
    framework::session_key::{SessionKey, SessionKeyModule, SessionScope},
};
use moveos_types::module_binding::MoveFunctionCaller;
use moveos_types::move_std::string::MoveString;

/// Create a new session key on-chain
#[derive(Debug, Parser)]
pub struct CreateCommand {
    #[clap(long)]
    pub app_name: MoveString,
    #[clap(long)]
    pub app_url: MoveString,

    /// The scope of the session key, format: address::module_name::function_name.
    /// The module_name and function_name must be valid Move identifiers or '*'. `*` means any module or function.
    /// For example: 0x3::empty::empty
    #[clap(long)]
    pub scope: SessionScope,

    /// The max inactive interval of the session key, in seconds.
    /// If the max_inactive_interval is 0, the session key will never expire.
    #[clap(long, default_value = "3600")]
    pub max_inactive_interval: u64,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

impl CreateCommand {
    pub async fn execute(self) -> KanariResult<SessionKey> {
        let mut context = self.context_options.build_require_password()?;

        let sender: KanariAddress = context.resolve_address(self.tx_options.sender)?.into();
        let max_gas_amount: Option<u64> = self.tx_options.max_gas_amount;

        let session_auth_key = context.generate_session_key(&sender)?;
        let session_scope = self.scope;

        let action =
            kanari_types::framework::session_key::SessionKeyModule::create_session_key_action(
                self.app_name,
                self.app_url,
                session_auth_key.as_ref().to_vec(),
                session_scope.clone(),
                self.max_inactive_interval,
            );

        println!("Generated new session key {session_auth_key} for address [{sender}]",);

        let tx_data = context
            .build_tx_data(sender, action, max_gas_amount)
            .await?;
        let result = context.sign_and_execute(sender, tx_data).await?;
        context.assert_execute_success(result)?;
        let client = context.get_client().await?;
        let session_key_module = client.as_module_binding::<SessionKeyModule>();
        let session_key = session_key_module
            .get_session_key(sender.into(), &session_auth_key)?
            .ok_or_else(|| {
                KanariError::ViewFunctionError(format!(
                    "Failed to get session key via {}",
                    session_auth_key
                ))
            })?;
        context
            .keystore
            .binding_session_key(sender, session_key.clone())?;
        Ok(session_key)
    }
}
