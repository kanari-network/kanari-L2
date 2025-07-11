// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, TransactionOptions, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_rpc_api::jsonrpc_types::TransactionExecutionInfoView;
use kanari_types::address::KanariAddress;
use kanari_types::error::KanariResult;
use kanari_types::framework::did::{DID, DIDModule, VerificationRelationship};
use kanari_types::transaction::KanariTransaction;
use kanari_types::transaction::authenticator::SessionAuthenticator;
use moveos_types::module_binding::MoveFunctionCaller;
use moveos_types::move_std::string::MoveString;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Manage DID operations (verification methods, services, etc.)
#[derive(Debug, Parser)]
pub struct ManageCommand {
    #[clap(subcommand)]
    pub manage_type: ManageType,
}

#[derive(Debug, Parser)]
pub enum ManageType {
    /// Add a verification method to the DID document
    #[clap(name = "add-vm")]
    AddVerificationMethod(AddVerificationMethodCommand),

    /// Remove a verification method from the DID document
    #[clap(name = "remove-vm")]
    RemoveVerificationMethod(RemoveVerificationMethodCommand),

    /// Add verification method to a relationship
    #[clap(name = "add-relationship")]
    AddToRelationship(AddToRelationshipCommand),

    /// Remove verification method from a relationship
    #[clap(name = "remove-relationship")]
    RemoveFromRelationship(RemoveFromRelationshipCommand),

    /// Add a service to the DID document
    #[clap(name = "add-service")]
    AddService(AddServiceCommand),

    /// Update an existing service
    #[clap(name = "update-service")]
    UpdateService(UpdateServiceCommand),

    /// Remove a service from the DID document
    #[clap(name = "remove-service")]
    RemoveService(RemoveServiceCommand),
}

#[derive(Debug, Parser)]
pub struct AddVerificationMethodCommand {
    /// DID address to operate on (the DID document address)
    #[clap(long, help = "DID address to operate on")]
    pub did_address: Option<String>,

    /// Fragment identifier for the verification method (e.g., "key-2")
    #[clap(long, help = "Fragment identifier for the verification method")]
    pub fragment: String,

    /// Type of verification method
    #[clap(
        long,
        default_value = "Ed25519VerificationKey2020",
        help = "Verification method type"
    )]
    pub method_type: String,

    /// Public key in multibase format
    #[clap(
        long,
        help = "Public key in multibase format, if not provided, will automatically generate a new key"
    )]
    pub public_key: Option<String>,

    /// Verification relationships (comma-separated)
    #[clap(
        long,
        help = "Verification relationships: auth,assert,invoke,delegate,agreement"
    )]
    pub relationships: Option<String>,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[derive(Debug, Parser)]
pub struct RemoveVerificationMethodCommand {
    /// DID address to operate on (the DID document address)
    #[clap(long, help = "DID address to operate on")]
    pub did_address: Option<String>,

    /// Fragment identifier of the verification method to remove
    #[clap(
        long,
        help = "Fragment identifier of the verification method to remove"
    )]
    pub fragment: String,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[derive(Debug, Parser)]
pub struct AddToRelationshipCommand {
    /// DID address to operate on (the DID document address)
    #[clap(long, help = "DID address to operate on")]
    pub did_address: Option<String>,

    /// Fragment identifier of the verification method
    #[clap(long, help = "Fragment identifier of the verification method")]
    pub fragment: String,

    /// Verification relationship to add to
    #[clap(long, help = "Relationship: auth, assert, invoke, delegate, agreement")]
    pub relationship: String,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[derive(Debug, Parser)]
pub struct RemoveFromRelationshipCommand {
    /// DID address to operate on (the DID document address)
    #[clap(long, help = "DID address to operate on")]
    pub did_address: Option<String>,

    /// Fragment identifier of the verification method
    #[clap(long, help = "Fragment identifier of the verification method")]
    pub fragment: String,

    /// Verification relationship to remove from
    #[clap(long, help = "Relationship: auth, assert, invoke, delegate, agreement")]
    pub relationship: String,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[derive(Debug, Parser)]
pub struct AddServiceCommand {
    /// DID address to operate on (the DID document address)
    #[clap(long, help = "DID address to operate on")]
    pub did_address: Option<String>,

    /// Fragment identifier for the service (e.g., "messaging")
    #[clap(long, help = "Fragment identifier for the service")]
    pub fragment: String,

    /// Type of service
    #[clap(long, help = "Service type (e.g., MessagingService)")]
    pub service_type: String,

    /// Service endpoint URL
    #[clap(long, help = "Service endpoint URL")]
    pub endpoint: String,

    /// Additional properties (key=value format, comma-separated)
    #[clap(long, help = "Additional properties: key1=value1,key2=value2")]
    pub properties: Option<String>,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[derive(Debug, Parser)]
pub struct UpdateServiceCommand {
    /// DID address to operate on (the DID document address)
    #[clap(long, help = "DID address to operate on")]
    pub did_address: Option<String>,

    /// Fragment identifier of the service to update
    #[clap(long, help = "Fragment identifier of the service to update")]
    pub fragment: String,

    /// New service type
    #[clap(long, help = "New service type")]
    pub service_type: String,

    /// New service endpoint URL
    #[clap(long, help = "New service endpoint URL")]
    pub endpoint: String,

    /// New properties (key=value format, comma-separated)
    #[clap(long, help = "New properties: key1=value1,key2=value2")]
    pub properties: Option<String>,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[derive(Debug, Parser)]
pub struct RemoveServiceCommand {
    /// DID address to operate on (the DID document address)
    #[clap(long, help = "DID address to operate on")]
    pub did_address: Option<String>,

    /// Fragment identifier of the service to remove
    #[clap(long, help = "Fragment identifier of the service to remove")]
    pub fragment: String,

    #[clap(flatten)]
    pub tx_options: TransactionOptions,

    #[clap(flatten)]
    pub context_options: WalletContextOptions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManageOutput {
    pub operation: String,
    pub did: String,
    pub did_address: KanariAddress,
    pub fragment: String,
    pub execution_info: TransactionExecutionInfoView,
}

#[async_trait]
impl CommandAction<ManageOutput> for ManageCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        match self.manage_type {
            ManageType::AddVerificationMethod(cmd) => cmd.execute().await,
            ManageType::RemoveVerificationMethod(cmd) => cmd.execute().await,
            ManageType::AddToRelationship(cmd) => cmd.execute().await,
            ManageType::RemoveFromRelationship(cmd) => cmd.execute().await,
            ManageType::AddService(cmd) => cmd.execute().await,
            ManageType::UpdateService(cmd) => cmd.execute().await,
            ManageType::RemoveService(cmd) => cmd.execute().await,
        }
    }
}

#[async_trait]
impl CommandAction<ManageOutput> for AddVerificationMethodCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        let mut context = self.context_options.build_require_password()?;
        let client = context.get_client().await?;
        let did_module = client.as_module_binding::<DIDModule>();

        // Resolve the DID address to operate on
        let did_address_str = self.did_address.ok_or_else(|| {
            kanari_types::error::KanariError::CommandArgumentError(
                "DID address is required".to_string(),
            )
        })?;
        let did_address = KanariAddress::from_str(&did_address_str)?;

        // Get DID document to find controller
        let did_document = did_module.get_did_document_by_address(did_address.into())?;

        let controllers = did_document.controller;
        if controllers.is_empty() {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!("DID {} has no controllers", did_address_str),
            ));
        }
        // For simplicity, use the first controller. In a real scenario, might need to select one.
        let controller_did_struct: DID = controllers[0].clone();
        let controller_address =
            KanariAddress::from_str(controller_did_struct.identifier.as_str())?;

        // Check if keystore contains the controller's key
        if !context.keystore.contains_address(&controller_address) {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Keystore does not contain key for controller {}",
                    controller_address
                ),
            ));
        }

        // Parse verification relationships
        let relationships = if let Some(rel_str) = &self.relationships {
            parse_verification_relationships(rel_str)?
        } else {
            vec![] // No relationships by default
        };

        // Create the action
        let fragment = MoveString::from_str(&self.fragment)?;
        let mut method_type = MoveString::from_str(&self.method_type)?;
        let public_key = if let Some(public_key) = &self.public_key {
            MoveString::from_str(public_key)?
        } else {
            let auth_key = context.generate_session_key(&did_address)?;
            let kp = context
                .get_session_key(&did_address, &auth_key)?
                .ok_or_else(|| {
                    kanari_types::error::KanariError::CommandArgumentError(
                        "Failed to get session key".to_string(),
                    )
                })?;
            method_type = MoveString::from_str("Ed25519VerificationKey2020")?;
            MoveString::from_str(&kp.public().raw_to_multibase())?
        };

        let action = DIDModule::add_verification_method_action(
            fragment,
            method_type,
            public_key,
            relationships,
        );

        // Build transaction data with DID address as sender
        let tx_data = context
            .build_tx_data(did_address, action, self.tx_options.max_gas_amount)
            .await?;

        // Sign transaction with controller's key
        let kp = context.get_key_pair(&controller_address)?;
        let authenticator = SessionAuthenticator::sign(&kp, &tx_data);
        let tx = KanariTransaction::new(tx_data, authenticator.into());
        let result = context.execute(tx).await?;

        context.assert_execute_success(result.clone())?;

        Ok(ManageOutput {
            operation: "add_verification_method".to_string(),
            did: format!("did:kanari:{}", did_address.to_bech32()),
            did_address,
            fragment: self.fragment,
            execution_info: result.execution_info,
        })
    }
}

#[async_trait]
impl CommandAction<ManageOutput> for RemoveVerificationMethodCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        let context = self.context_options.build_require_password()?;
        let client = context.get_client().await?;
        let did_module = client.as_module_binding::<DIDModule>();

        let did_address_str = self.did_address.ok_or_else(|| {
            kanari_types::error::KanariError::CommandArgumentError(
                "DID address is required".to_string(),
            )
        })?;
        let did_address = KanariAddress::from_str(&did_address_str)?;

        let did_document = did_module.get_did_document_by_address(did_address.into())?;
        let controllers = did_document.controller;
        if controllers.is_empty() {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!("DID {} has no controllers", did_address_str),
            ));
        }
        let controller_did_struct: DID = controllers[0].clone();
        let controller_address =
            KanariAddress::from_str(controller_did_struct.identifier.as_str())?;

        if !context.keystore.contains_address(&controller_address) {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Keystore does not contain key for controller {}",
                    controller_address
                ),
            ));
        }

        let fragment = MoveString::from_str(&self.fragment)?;
        let action = DIDModule::remove_verification_method_action(fragment);

        let tx_data = context
            .build_tx_data(did_address, action, self.tx_options.max_gas_amount)
            .await?;

        // Sign transaction with controller's key
        let kp = context.get_key_pair(&controller_address)?;
        let authenticator = SessionAuthenticator::sign(&kp, &tx_data);
        let tx = KanariTransaction::new(tx_data, authenticator.into());
        let result = context.execute(tx).await?;

        context.assert_execute_success(result.clone())?;

        Ok(ManageOutput {
            operation: "remove_verification_method".to_string(),
            did: format!("did:kanari:{}", did_address.to_bech32()),
            did_address,
            fragment: self.fragment,
            execution_info: result.execution_info,
        })
    }
}

#[async_trait]
impl CommandAction<ManageOutput> for AddToRelationshipCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        let context = self.context_options.build_require_password()?;
        let client = context.get_client().await?;
        let did_module = client.as_module_binding::<DIDModule>();

        let did_address_str = self.did_address.ok_or_else(|| {
            kanari_types::error::KanariError::CommandArgumentError(
                "DID address is required".to_string(),
            )
        })?;
        let did_address = KanariAddress::from_str(&did_address_str)?;

        let did_document = did_module.get_did_document_by_address(did_address.into())?;
        let controllers = did_document.controller;
        if controllers.is_empty() {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!("DID {} has no controllers", did_address_str),
            ));
        }
        let controller_did_struct: DID = controllers[0].clone();
        let controller_address =
            KanariAddress::from_str(controller_did_struct.identifier.as_str())?;

        if !context.keystore.contains_address(&controller_address) {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Keystore does not contain key for controller {}",
                    controller_address
                ),
            ));
        }

        let relationship = VerificationRelationship::from_string(&self.relationship)?;
        let fragment = MoveString::from_str(&self.fragment)?;

        let action =
            DIDModule::add_to_verification_relationship_action(fragment, relationship as u8);

        let tx_data = context
            .build_tx_data(did_address, action, self.tx_options.max_gas_amount)
            .await?;

        // Sign transaction with controller's key
        let kp = context.get_key_pair(&controller_address)?;
        let authenticator = SessionAuthenticator::sign(&kp, &tx_data);
        let tx = KanariTransaction::new(tx_data, authenticator.into());
        let result = context.execute(tx).await?;

        context.assert_execute_success(result.clone())?;

        Ok(ManageOutput {
            operation: format!("add_to_{}", self.relationship),
            did: format!("did:kanari:{}", did_address.to_bech32()),
            did_address,
            fragment: self.fragment,
            execution_info: result.execution_info,
        })
    }
}

#[async_trait]
impl CommandAction<ManageOutput> for RemoveFromRelationshipCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        let context = self.context_options.build_require_password()?;
        let client = context.get_client().await?;
        let did_module = client.as_module_binding::<DIDModule>();

        let did_address_str = self.did_address.ok_or_else(|| {
            kanari_types::error::KanariError::CommandArgumentError(
                "DID address is required".to_string(),
            )
        })?;
        let did_address = KanariAddress::from_str(&did_address_str)?;

        let did_document = did_module.get_did_document_by_address(did_address.into())?;
        let controllers = did_document.controller;
        if controllers.is_empty() {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!("DID {} has no controllers", did_address_str),
            ));
        }
        let controller_did_struct: DID = controllers[0].clone();
        let controller_address =
            KanariAddress::from_str(controller_did_struct.identifier.as_str())?;

        if !context.keystore.contains_address(&controller_address) {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Keystore does not contain key for controller {}",
                    controller_address
                ),
            ));
        }

        let relationship = VerificationRelationship::from_string(&self.relationship)?;
        let fragment = MoveString::from_str(&self.fragment)?;

        let action =
            DIDModule::remove_from_verification_relationship_action(fragment, relationship as u8);

        let tx_data = context
            .build_tx_data(did_address, action, self.tx_options.max_gas_amount)
            .await?;

        // Sign transaction with controller's key
        let kp = context.get_key_pair(&controller_address)?;
        let authenticator = SessionAuthenticator::sign(&kp, &tx_data);
        let tx = KanariTransaction::new(tx_data, authenticator.into());
        let result = context.execute(tx).await?;

        context.assert_execute_success(result.clone())?;

        Ok(ManageOutput {
            operation: format!("remove_from_{}", self.relationship),
            did: format!("did:kanari:{}", did_address.to_bech32()),
            did_address,
            fragment: self.fragment,
            execution_info: result.execution_info,
        })
    }
}

#[async_trait]
impl CommandAction<ManageOutput> for AddServiceCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        let context = self.context_options.build_require_password()?;
        let client = context.get_client().await?;
        let did_module = client.as_module_binding::<DIDModule>();

        let did_address_str = self.did_address.ok_or_else(|| {
            kanari_types::error::KanariError::CommandArgumentError(
                "DID address is required".to_string(),
            )
        })?;
        let did_address = KanariAddress::from_str(&did_address_str)?;

        let did_document = did_module.get_did_document_by_address(did_address.into())?;
        let controllers = did_document.controller;
        if controllers.is_empty() {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!("DID {} has no controllers", did_address_str),
            ));
        }
        let controller_did_struct: DID = controllers[0].clone();
        let controller_address =
            KanariAddress::from_str(controller_did_struct.identifier.as_str())?;

        if !context.keystore.contains_address(&controller_address) {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Keystore does not contain key for controller {}",
                    controller_address
                ),
            ));
        }

        let fragment = MoveString::from_str(&self.fragment)?;
        let service_type = MoveString::from_str(&self.service_type)?;
        let endpoint = MoveString::from_str(&self.endpoint)?;

        let action = if let Some(props_str) = &self.properties {
            let (keys, values) = parse_properties(props_str)?;
            DIDModule::add_service_with_properties_action(
                fragment,
                service_type,
                endpoint,
                keys,
                values,
            )
        } else {
            DIDModule::add_service_action(fragment, service_type, endpoint)
        };

        let tx_data = context
            .build_tx_data(did_address, action, self.tx_options.max_gas_amount)
            .await?;

        // Sign transaction with controller's key
        let kp = context.get_key_pair(&controller_address)?;
        let authenticator = SessionAuthenticator::sign(&kp, &tx_data);
        let tx = KanariTransaction::new(tx_data, authenticator.into());
        let result = context.execute(tx).await?;

        context.assert_execute_success(result.clone())?;

        Ok(ManageOutput {
            operation: "add_service".to_string(),
            did: format!("did:kanari:{}", did_address.to_bech32()),
            did_address,
            fragment: self.fragment,
            execution_info: result.execution_info,
        })
    }
}

#[async_trait]
impl CommandAction<ManageOutput> for UpdateServiceCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        let context = self.context_options.build_require_password()?;
        let client = context.get_client().await?;
        let did_module = client.as_module_binding::<DIDModule>();

        let did_address_str = self.did_address.ok_or_else(|| {
            kanari_types::error::KanariError::CommandArgumentError(
                "DID address is required".to_string(),
            )
        })?;
        let did_address = KanariAddress::from_str(&did_address_str)?;

        let did_document = did_module.get_did_document_by_address(did_address.into())?;
        let controllers = did_document.controller;
        if controllers.is_empty() {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!("DID {} has no controllers", did_address_str),
            ));
        }
        let controller_did_struct: DID = controllers[0].clone();
        let controller_address =
            KanariAddress::from_str(controller_did_struct.identifier.as_str())?;

        if !context.keystore.contains_address(&controller_address) {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Keystore does not contain key for controller {}",
                    controller_address
                ),
            ));
        }

        let fragment = MoveString::from_str(&self.fragment)?;
        let service_type = MoveString::from_str(&self.service_type)?;
        let endpoint = MoveString::from_str(&self.endpoint)?;

        let (keys, values) = if let Some(props_str) = &self.properties {
            parse_properties(props_str)?
        } else {
            (vec![], vec![])
        };

        let action =
            DIDModule::update_service_action(fragment, service_type, endpoint, keys, values);

        let tx_data = context
            .build_tx_data(did_address, action, self.tx_options.max_gas_amount)
            .await?;

        // Sign transaction with controller's key
        let kp = context.get_key_pair(&controller_address)?;
        let authenticator = SessionAuthenticator::sign(&kp, &tx_data);
        let tx = KanariTransaction::new(tx_data, authenticator.into());
        let result = context.execute(tx).await?;

        context.assert_execute_success(result.clone())?;

        Ok(ManageOutput {
            operation: "update_service".to_string(),
            did: format!("did:kanari:{}", did_address.to_bech32()),
            did_address,
            fragment: self.fragment,
            execution_info: result.execution_info,
        })
    }
}

#[async_trait]
impl CommandAction<ManageOutput> for RemoveServiceCommand {
    async fn execute(self) -> KanariResult<ManageOutput> {
        let context = self.context_options.build_require_password()?;
        let client = context.get_client().await?;
        let did_module = client.as_module_binding::<DIDModule>();

        let did_address_str = self.did_address.ok_or_else(|| {
            kanari_types::error::KanariError::CommandArgumentError(
                "DID address is required".to_string(),
            )
        })?;
        let did_address = KanariAddress::from_str(&did_address_str)?;

        let did_document = did_module.get_did_document_by_address(did_address.into())?;
        let controllers = did_document.controller;
        if controllers.is_empty() {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!("DID {} has no controllers", did_address_str),
            ));
        }
        let controller_did_struct: DID = controllers[0].clone();
        let controller_address =
            KanariAddress::from_str(controller_did_struct.identifier.as_str())?;

        if !context.keystore.contains_address(&controller_address) {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Keystore does not contain key for controller {}",
                    controller_address
                ),
            ));
        }

        let fragment = MoveString::from_str(&self.fragment)?;
        let action = DIDModule::remove_service_action(fragment);

        let tx_data = context
            .build_tx_data(did_address, action, self.tx_options.max_gas_amount)
            .await?;

        // Sign transaction with controller's key
        let kp = context.get_key_pair(&controller_address)?;
        let authenticator = SessionAuthenticator::sign(&kp, &tx_data);
        let tx = KanariTransaction::new(tx_data, authenticator.into());
        let result = context.execute(tx).await?;

        context.assert_execute_success(result.clone())?;

        Ok(ManageOutput {
            operation: "remove_service".to_string(),
            did: format!("did:kanari:{}", did_address.to_bech32()),
            did_address,
            fragment: self.fragment,
            execution_info: result.execution_info,
        })
    }
}

/// Parse verification relationships from a comma-separated string
fn parse_verification_relationships(relationships_str: &str) -> KanariResult<Vec<u8>> {
    let mut relationships = Vec::new();

    for rel_str in relationships_str.split(',') {
        let rel_str = rel_str.trim();
        if rel_str.is_empty() {
            continue;
        }

        let relationship = VerificationRelationship::from_string(rel_str).map_err(|e| {
            kanari_types::error::KanariError::CommandArgumentError(format!(
                "Invalid verification relationship '{}': {}",
                rel_str, e
            ))
        })?;
        relationships.push(relationship as u8);
    }

    Ok(relationships)
}

/// Parse properties from a comma-separated key=value string
fn parse_properties(properties_str: &str) -> KanariResult<(Vec<MoveString>, Vec<MoveString>)> {
    let mut keys = Vec::new();
    let mut values = Vec::new();

    for prop_str in properties_str.split(',') {
        let prop_str = prop_str.trim();
        if prop_str.is_empty() {
            continue;
        }

        let parts: Vec<&str> = prop_str.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(kanari_types::error::KanariError::CommandArgumentError(
                format!(
                    "Invalid property format '{}'. Expected 'key=value'",
                    prop_str
                ),
            ));
        }

        keys.push(MoveString::from_str(parts[0].trim())?);
        values.push(MoveString::from_str(parts[1].trim())?);
    }

    Ok((keys, values))
}
