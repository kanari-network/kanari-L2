// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use super::RawTransaction;
use super::authenticator::{BitcoinAuthenticator, BitcoinMultisignAuthenticator};
use super::{AuthenticatorInfo, authenticator::Authenticator};
use crate::address::KanariAddress;
use crate::crypto::KanariKeyPair;
use crate::kanari_network::BuiltinChainID;
use anyhow::Result;
use moveos_types::h256::H256;
use moveos_types::moveos_std::gas_schedule::GasScheduleConfig;
use moveos_types::moveos_std::object::ObjectMeta;
use moveos_types::{
    moveos_std::tx_context::TxContext,
    transaction::{MoveAction, MoveOSTransaction},
};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct KanariTransactionData {
    /// Sender's address.
    pub sender: KanariAddress,
    // Sequence number of this transaction corresponding to sender's account.
    pub sequence_number: u64,
    // The ChainID of the transaction.
    pub chain_id: u64,
    // The max gas to be used.
    pub max_gas_amount: u64,
    // The MoveAction to execute.
    pub action: MoveAction,
}

impl KanariTransactionData {
    pub fn new(
        sender: KanariAddress,
        sequence_number: u64,
        chain_id: u64,
        max_gas_amount: u64,
        action: MoveAction,
    ) -> Self {
        Self {
            sender,
            sequence_number,
            chain_id,
            max_gas_amount,
            action,
        }
    }

    pub fn new_for_test(sender: KanariAddress, sequence_number: u64, action: MoveAction) -> Self {
        Self {
            sender,
            sequence_number,
            chain_id: BuiltinChainID::Local.chain_id().id(),
            max_gas_amount: GasScheduleConfig::INITIAL_MAX_GAS_AMOUNT,
            action,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("encode transaction should success")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self>
    where
        Self: std::marker::Sized,
    {
        bcs::from_bytes::<Self>(bytes).map_err(Into::into)
    }

    pub fn tx_hash(&self) -> H256 {
        moveos_types::h256::sha3_256_of(self.encode().as_slice())
    }

    pub fn tx_size(&self) -> u64 {
        bcs::serialized_size(self).expect("serialize transaction size should success") as u64
    }

    pub fn sign(&self, kp: &KanariKeyPair) -> KanariTransaction {
        let auth = Authenticator::sign(kp, self);
        KanariTransaction::new(self.clone(), auth)
    }
}

impl Display for KanariTransactionData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ sender: {}, sequence_number {}, chain_id: {}, max_gas_amount: {}, action: {} }}",
            self.sender, self.sequence_number, self.chain_id, self.max_gas_amount, self.action
        )
    }
}

/// PartiallySignedKanariTransaction(PSRT) is a transaction that has been signed by partial signers.
/// It can be used for multi-signatures.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PartiallySignedKanariTransaction {
    pub data: KanariTransactionData,
    /// The threshold of the signatures.
    pub threshold: u64,
    /// The signatures of the partial signers.
    pub authenticators: Vec<BitcoinAuthenticator>,
}

impl PartiallySignedKanariTransaction {
    pub fn new(data: KanariTransactionData, threshold: u64) -> Self {
        Self {
            data,
            threshold,
            authenticators: vec![],
        }
    }

    pub fn sender(&self) -> KanariAddress {
        self.data.sender
    }

    pub fn signatories(&self) -> usize {
        self.authenticators.len()
    }

    pub fn contains_authenticator(&self, authenticator: &BitcoinAuthenticator) -> bool {
        self.authenticators
            .iter()
            .any(|a| a.payload.public_key == authenticator.payload.public_key)
    }

    pub fn add_authenticator(&mut self, authenticator: BitcoinAuthenticator) -> Result<()> {
        if self.contains_authenticator(&authenticator) {
            return Err(anyhow::anyhow!(
                "Authenticator from address {:?} already exists",
                authenticator.payload.from_address()
            ));
        }
        self.authenticators.push(authenticator);
        Ok(())
    }

    pub fn threshold(&self) -> u64 {
        self.threshold
    }

    pub fn is_fully_signed(&self) -> bool {
        self.authenticators.len() as u64 >= self.threshold
    }

    pub fn try_into_kanari_transaction(self) -> Result<KanariTransaction> {
        if !self.is_fully_signed() {
            return Err(anyhow::anyhow!(
                "Not enough signatures to complete transaction"
            ));
        }

        let authenticator =
            BitcoinMultisignAuthenticator::build_multisig_authenticator(self.authenticators)?
                .into();
        Ok(KanariTransaction::new(self.data, authenticator))
    }

    pub fn encode(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("encode transaction should success")
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct KanariTransaction {
    pub data: KanariTransactionData,
    pub authenticator: Authenticator,

    #[serde(skip_serializing, skip_deserializing)]
    data_hash: Option<H256>,
}

impl KanariTransaction {
    pub fn new(data: KanariTransactionData, authenticator: Authenticator) -> Self {
        Self {
            data,
            authenticator,
            data_hash: None,
        }
    }

    pub fn new_genesis_tx(
        genesis_address: KanariAddress,
        chain_id: u64,
        action: MoveAction,
    ) -> Self {
        Self {
            data: KanariTransactionData::new(genesis_address, 0, chain_id, u64::MAX, action),
            authenticator: Authenticator::genesis(),
            data_hash: None,
        }
    }

    pub fn sender(&self) -> KanariAddress {
        self.data.sender
    }

    pub fn sequence_number(&self) -> u64 {
        self.data.sequence_number
    }

    pub fn chain_id(&self) -> u64 {
        self.data.chain_id
    }

    pub fn max_gas_amount(&self) -> u64 {
        self.data.max_gas_amount
    }

    pub fn action(&self) -> &MoveAction {
        &self.data.action
    }

    pub fn decode(bytes: &[u8]) -> Result<Self>
    where
        Self: std::marker::Sized,
    {
        bcs::from_bytes::<Self>(bytes).map_err(Into::into)
    }

    pub fn encode(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("encode transaction should success")
    }

    pub fn tx_hash(&mut self) -> H256 {
        if let Some(hash) = self.data_hash {
            hash
        } else {
            let hash = self.data.tx_hash();
            self.data_hash = Some(hash);
            hash
        }
    }

    pub fn authenticator_info(&self) -> AuthenticatorInfo {
        AuthenticatorInfo::new(self.chain_id(), self.authenticator.clone())
    }

    pub fn authenticator(&self) -> &Authenticator {
        &self.authenticator
    }

    pub fn tx_size(&self) -> u64 {
        bcs::serialized_size(self).expect("serialize transaction size should success") as u64
    }

    //TODO use protest Arbitrary to generate mock data
    pub fn mock() -> KanariTransaction {
        use crate::address::KanariSupportedAddress;
        use move_core_types::{
            account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
        };
        use moveos_types::move_types::FunctionId;

        let sender: KanariAddress = KanariAddress::random();
        let sequence_number = 0;
        let payload = MoveAction::new_function_call(
            FunctionId::new(
                ModuleId::new(AccountAddress::random(), Identifier::new("test").unwrap()),
                Identifier::new("test").unwrap(),
            ),
            vec![],
            vec![],
        );

        let transaction_data =
            KanariTransactionData::new_for_test(sender, sequence_number, payload);

        let kp = &KanariKeyPair::generate_secp256k1();
        let auth = Authenticator::bitcoin(kp, &transaction_data);

        KanariTransaction::new(transaction_data, auth)
    }

    pub fn into_moveos_transaction(mut self, root: ObjectMeta) -> MoveOSTransaction {
        let tx_hash = self.tx_hash();
        let tx_size = self.tx_size();
        let tx_ctx = TxContext::new(
            self.data.sender.into(),
            self.data.sequence_number,
            self.data.max_gas_amount,
            tx_hash,
            tx_size,
        );
        MoveOSTransaction::new(root, tx_ctx, self.data.action)
    }
}

impl TryFrom<RawTransaction> for KanariTransaction {
    type Error = anyhow::Error;

    fn try_from(raw: RawTransaction) -> Result<Self> {
        let tx = KanariTransaction::decode(raw.raw.as_slice())?;
        Ok(tx)
    }
}

impl Display for KanariTransaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "KanariTransaction {{ data: {}, authenticator: {}, data_hash {:?} }}",
            self.data, self.authenticator, self.data_hash
        )
    }
}
