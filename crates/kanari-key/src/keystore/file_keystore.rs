// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use super::types::LocalAccount;
use crate::keystore::account_keystore::AccountKeystore;
use crate::keystore::base_keystore::BaseKeyStore;
use anyhow::anyhow;
use kanari_types::key_struct::{MnemonicData, MnemonicResult};
use kanari_types::{
    address::KanariAddress,
    authentication_key::AuthenticationKey,
    crypto::{KanariKeyPair, Signature},
    key_struct::EncryptionData,
    transaction::kanari::{KanariTransaction, KanariTransactionData},
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct FileBasedKeystore {
    pub(crate) keystore: BaseKeyStore,
    pub(crate) path: Option<PathBuf>,
}

impl AccountKeystore for FileBasedKeystore {
    fn init_mnemonic_data(&mut self, mnemonic_data: MnemonicData) -> Result<(), anyhow::Error> {
        self.keystore.init_mnemonic_data(mnemonic_data)?;
        self.save()?;
        Ok(())
    }

    fn add_addresses_to_mnemonic_data(
        &mut self,
        address: KanariAddress,
    ) -> Result<(), anyhow::Error> {
        self.keystore.add_addresses_to_mnemonic_data(address)?;
        self.save()?;
        Ok(())
    }

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
            .add_address_encryption_data_to_keys(address, encryption)?;
        self.save()?;
        Ok(())
    }

    fn get_key_pair(
        &self,
        address: &KanariAddress,
        password: Option<String>,
    ) -> Result<KanariKeyPair, anyhow::Error> {
        self.keystore.get_key_pair(address, password)
    }

    fn nullify(&mut self, address: &KanariAddress) -> Result<(), anyhow::Error> {
        self.keystore.nullify(address)?;
        self.save()?;
        Ok(())
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
        let auth_key = self.keystore.generate_session_key(address, password)?;
        self.save()?;
        Ok(auth_key)
    }

    fn binding_session_key(
        &mut self,
        address: KanariAddress,
        session_key: kanari_types::framework::session_key::SessionKey,
    ) -> Result<(), anyhow::Error> {
        self.keystore.binding_session_key(address, session_key)?;
        self.save()?;
        Ok(())
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
        self.save()?;
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
}

impl FileBasedKeystore {
    pub fn new(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let keystore = if path.exists() {
            let reader = BufReader::new(File::open(path).map_err(|e| {
                anyhow!(
                    "Can't open FileBasedKeystore from Kanari path {:?}: {}",
                    path,
                    e
                )
            })?);
            serde_json::from_reader(reader).map_err(|e| {
                anyhow!(
                    "Can't deserialize FileBasedKeystore from Kanari path {:?}: {}",
                    path,
                    e
                )
            })?
        } else {
            BaseKeyStore::new()
        };

        Ok(Self {
            keystore,
            path: Some(path.to_path_buf()),
        })
    }

    pub fn load(path: &PathBuf) -> Result<Self, anyhow::Error> {
        if path.exists() {
            let reader = BufReader::new(File::open(path).map_err(|e| {
                anyhow!(
                    "Can't open FileBasedKeystore from Kanari path {:?}: {}",
                    path,
                    e
                )
            })?);
            let keystore = serde_json::from_reader(reader).map_err(|e| {
                anyhow!(
                    "Can't deserialize FileBasedKeystore from Kanari path {:?}: {}",
                    path,
                    e
                )
            })?;
            Ok(Self {
                keystore,
                path: Some(path.to_path_buf()),
            })
        } else {
            Err(anyhow!("Key store path {:?} does not exist", path))
        }
    }

    pub fn set_path(&mut self, path: &Path) {
        self.path = Some(path.to_path_buf());
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        if let Some(path) = &self.path {
            let store = serde_json::to_string_pretty(&self.keystore)?;
            fs::write(path, store)?;
        }
        Ok(())
    }

    pub fn key_pairs(
        &self,
        _address: &KanariAddress,
        password: Option<String>,
    ) -> Result<Vec<KanariKeyPair>, anyhow::Error> {
        // Collect references to KanariKeyPair objects from all inner maps.
        let key_pairs: Vec<KanariKeyPair> = self
            .keystore
            .keys
            .values() // Get inner maps
            .flat_map(|encryption| {
                Some(encryption.decrypt_with_type::<KanariKeyPair>(password.clone()))
            })
            .collect::<Result<_, _>>()?;

        Ok(key_pairs)
    }
}
