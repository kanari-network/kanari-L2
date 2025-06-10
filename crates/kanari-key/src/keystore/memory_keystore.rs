// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use super::types::LocalAccount;
use crate::keystore::account_keystore::AccountKeystore;
use crate::keystore::base_keystore::BaseKeyStore;
use kanari_types::key_struct::{MnemonicData, MnemonicResult};
use kanari_types::{
    address::KanariAddress,
    authentication_key::AuthenticationKey,
    crypto::{KanariKeyPair, Signature},
    key_struct::EncryptionData,
    transaction::kanari::{KanariTransaction, KanariTransactionData},
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct InMemKeystore {
    keystore: BaseKeyStore,
}

impl AccountKeystore for InMemKeystore {
    fn get_accounts(&self, password: Option<String>) -> Result<Vec<LocalAccount>, anyhow::Error> {
        self.keystore.get_accounts(password)
    }

    fn contains_address(&self, address: &KanariAddress) -> bool {
        self.keystore.contains_address(address)
    }

    fn add_address_encryption_data_to_keys(
        &mut self,
        address: KanariAddress,
        encryption: EncryptionData,
    ) -> Result<(), anyhow::Error> {
        self.keystore
            .add_address_encryption_data_to_keys(address, encryption)
    }

    fn get_key_pair(
        &self,
        address: &KanariAddress,
        password: Option<String>,
    ) -> Result<KanariKeyPair, anyhow::Error> {
        self.keystore.get_key_pair(address, password)
    }

    fn nullify(&mut self, address: &KanariAddress) -> Result<(), anyhow::Error> {
        self.keystore.nullify(address)
    }

    fn sign_hashed(
        &self,
        address: &KanariAddress,
        msg: &[u8],
        password: Option<String>,
    ) -> Result<Signature, anyhow::Error> {
        self.keystore.sign_hashed(address, msg, password)
    }

    fn sign_transaction(
        &self,
        address: &KanariAddress,
        msg: KanariTransactionData,
        password: Option<String>,
    ) -> Result<KanariTransaction, anyhow::Error> {
        self.keystore.sign_transaction(address, msg, password)
    }

    fn sign_secure<T>(
        &self,
        address: &KanariAddress,
        msg: &T,
        password: Option<String>,
    ) -> Result<Signature, anyhow::Error>
    where
        T: Serialize,
    {
        self.keystore.sign_secure(address, msg, password)
    }

    fn addresses(&self) -> Vec<KanariAddress> {
        // Create an empty Vec to store the addresses.
        let mut addresses =
            Vec::with_capacity(self.keystore.keys.len() + self.keystore.session_keys.len());

        // Iterate over the `keys` and `session_keys` BTreeMaps.
        for key in self.keystore.keys.keys() {
            addresses.push(*key);
        }

        for key in self.keystore.session_keys.keys() {
            addresses.push(*key);
        }

        addresses
    }

    fn generate_session_key(
        &mut self,
        address: &KanariAddress,
        password: Option<String>,
    ) -> Result<AuthenticationKey, anyhow::Error> {
        self.keystore.generate_session_key(address, password)
    }

    fn binding_session_key(
        &mut self,
        address: KanariAddress,
        session_key: kanari_types::framework::session_key::SessionKey,
    ) -> Result<(), anyhow::Error> {
        self.keystore.binding_session_key(address, session_key)
    }

    fn get_session_key(
        &self,
        address: &KanariAddress,
        authentication_key: &AuthenticationKey,
        password: Option<String>,
    ) -> Result<Option<KanariKeyPair>, anyhow::Error> {
        self.keystore
            .get_session_key(address, authentication_key, password)
    }

    fn sign_transaction_via_session_key(
        &self,
        address: &KanariAddress,
        msg: KanariTransactionData,
        authentication_key: &AuthenticationKey,
        password: Option<String>,
    ) -> Result<KanariTransaction, anyhow::Error> {
        self.keystore
            .sign_transaction_via_session_key(address, msg, authentication_key, password)
    }

    fn set_password_hash_with_indicator(
        &mut self,
        password_hash: String,
        is_password_empty: bool,
    ) -> Result<(), anyhow::Error> {
        self.keystore.password_hash = Some(password_hash);
        self.keystore.is_password_empty = is_password_empty;
        Ok(())
    }

    fn get_password_hash(&self) -> String {
        self.keystore.password_hash.clone().unwrap_or_default()
    }

    fn get_if_password_is_empty(&self) -> bool {
        self.keystore.is_password_empty
    }

    fn get_mnemonic(&self, password: Option<String>) -> Result<MnemonicResult, anyhow::Error> {
        self.keystore.get_mnemonic(password)
    }

    fn init_mnemonic_data(&mut self, mnemonic_data: MnemonicData) -> Result<(), anyhow::Error> {
        self.keystore.init_mnemonic_data(mnemonic_data)
    }

    fn add_addresses_to_mnemonic_data(
        &mut self,
        address: KanariAddress,
    ) -> Result<(), anyhow::Error> {
        self.keystore.add_addresses_to_mnemonic_data(address)
    }
}

impl InMemKeystore {
    pub fn new_insecure_for_tests(initial_key_number: usize) -> Self {
        let mut keystore = BaseKeyStore::new();
        keystore.init_keystore(None, None, None).unwrap();
        for _ in 0..initial_key_number {
            keystore.generate_and_add_new_key(None).unwrap();
        }

        Self { keystore }
    }
}