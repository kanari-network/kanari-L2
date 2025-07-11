// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use super::types::{LocalAccount, LocalSessionKey};
use crate::keystore::account_keystore::AccountKeystore;
use anyhow::{Ok, ensure};
use kanari_types::framework::session_key::SessionKey;
use kanari_types::key_struct::{MnemonicData, MnemonicResult};
use kanari_types::to_bech32::ToBech32;
use kanari_types::{
    address::KanariAddress,
    authentication_key::AuthenticationKey,
    crypto::{KanariKeyPair, Signature},
    error::KanariError,
    key_struct::EncryptionData,
    transaction::{
        authenticator,
        kanari::{KanariTransaction, KanariTransactionData},
    },
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::BTreeMap;

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde_as]
pub(crate) struct BaseKeyStore {
    #[serde(default)]
    pub(crate) keys: BTreeMap<KanariAddress, EncryptionData>,
    #[serde(default)]
    pub(crate) mnemonic: Option<MnemonicData>,
    #[serde(default)]
    #[serde_as(as = "BTreeMap<DisplayFromStr, BTreeMap<DisplayFromStr, _>>")]
    pub(crate) session_keys: BTreeMap<KanariAddress, BTreeMap<AuthenticationKey, LocalSessionKey>>,
    #[serde(default)]
    pub(crate) password_hash: Option<String>,
    #[serde(default)]
    pub(crate) is_password_empty: bool,
}

impl BaseKeyStore {
    pub fn new() -> Self {
        Self {
            keys: BTreeMap::new(),
            mnemonic: None,
            session_keys: BTreeMap::new(),
            password_hash: None,
            is_password_empty: true,
        }
    }
}

impl AccountKeystore for BaseKeyStore {
    fn init_mnemonic_data(&mut self, mnemonic_data: MnemonicData) -> Result<(), anyhow::Error> {
        ensure!(self.mnemonic.is_none(), "Mnemonic data already exists");
        self.mnemonic = Some(mnemonic_data);
        Ok(())
    }

    fn add_addresses_to_mnemonic_data(
        &mut self,
        address: KanariAddress,
    ) -> Result<(), anyhow::Error> {
        ensure!(self.mnemonic.is_some(), "Mnemonic data do not exist");
        let mut mnemonic_unwrapped = self.mnemonic.clone().unwrap();
        ensure!(!mnemonic_unwrapped.addresses.is_empty(), "Address is empty");
        mnemonic_unwrapped.addresses.push(address);
        self.mnemonic = Some(mnemonic_unwrapped);
        Ok(())
    }

    fn get_accounts(&self, password: Option<String>) -> Result<Vec<LocalAccount>, anyhow::Error> {
        let mut accounts = BTreeMap::new();
        for (address, encryption) in &self.keys {
            let keypair: KanariKeyPair = encryption.decrypt_with_type(password.clone())?;
            let public_key = keypair.public();
            let bitcoin_address = public_key.bitcoin_address()?;
            let nostr_bech32_public_key = public_key.xonly_public_key()?.to_bech32()?;
            let has_session_key = self.session_keys.contains_key(address);
            let local_account = LocalAccount {
                address: *address,
                bitcoin_address,
                nostr_bech32_public_key,
                public_key,
                has_session_key,
            };
            accounts.insert(*address, local_account);
        }
        Ok(accounts.into_values().collect())
    }

    fn contains_address(&self, address: &KanariAddress) -> bool {
        self.keys.contains_key(address)
    }

    // TODO: deal with the Kanari and Nostr's get_key_pair() function. Consider Nostr scenario
    fn get_key_pair(
        &self,
        address: &KanariAddress,
        password: Option<String>,
    ) -> Result<KanariKeyPair, anyhow::Error> {
        if let Some(encryption) = self.keys.get(address) {
            let keypair: KanariKeyPair = encryption.decrypt_with_type::<KanariKeyPair>(password)?;
            Ok(keypair)
        } else {
            Err(anyhow::Error::new(KanariError::CommandArgumentError(
                format!("Cannot find key for address: [{:?}]", address),
            )))
        }
    }

    fn sign_hashed(
        &self,
        address: &KanariAddress,
        msg: &[u8],
        password: Option<String>,
    ) -> Result<Signature, anyhow::Error> {
        Ok(Signature::sign(msg, &self.get_key_pair(address, password)?))
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
        Ok(Signature::sign_secure(
            msg,
            &self.get_key_pair(address, password)?,
        ))
    }

    fn sign_transaction(
        &self,
        address: &KanariAddress,
        msg: KanariTransactionData,
        password: Option<String>,
    ) -> Result<KanariTransaction, anyhow::Error> {
        let kp = self.get_key_pair(address, password).ok().ok_or_else(|| {
            KanariError::SignMessageError(format!("Cannot find key for address: [{address}]"))
        })?;
        let auth = authenticator::Authenticator::bitcoin(&kp, &msg);
        Ok(KanariTransaction::new(msg, auth))
    }

    fn add_address_encryption_data_to_keys(
        &mut self,
        address: KanariAddress,
        encryption: EncryptionData,
    ) -> Result<(), anyhow::Error> {
        self.keys.entry(address).or_insert(encryption);
        Ok(())
    }

    fn nullify(&mut self, address: &KanariAddress) -> Result<(), anyhow::Error> {
        self.keys.remove(address);
        let mnemonic_data = match &self.mnemonic {
            Some(mnemonic) => mnemonic,
            // For None, this could be indicating that there's no internal account address in the mnemonic addresses
            None => return Ok(()),
        };
        match mnemonic_data
            .addresses
            .iter()
            .position(|&target_address| target_address == *address)
        {
            Some(index) => self.mnemonic.as_mut().unwrap().addresses.remove(index),
            // For None, this could be either non-existing address in keystore or the external account address
            None => return Ok(()),
        };
        Ok(())
    }

    fn generate_session_key(
        &mut self,
        address: &KanariAddress,
        password: Option<String>,
    ) -> Result<AuthenticationKey, anyhow::Error> {
        let kp: KanariKeyPair = KanariKeyPair::generate_ed25519();
        let authentication_key = kp.public().authentication_key();
        let inner_map = self.session_keys.entry(*address).or_default();
        let private_key_encryption = EncryptionData::encrypt_with_type(&kp, password)?;
        let local_session_key = LocalSessionKey {
            session_key: None,
            private_key: private_key_encryption,
        };
        inner_map.insert(authentication_key.clone(), local_session_key);
        Ok(authentication_key)
    }

    fn binding_session_key(
        &mut self,
        address: KanariAddress,
        session_key: SessionKey,
    ) -> Result<(), anyhow::Error> {
        let inner_map: &mut BTreeMap<AuthenticationKey, LocalSessionKey> =
            self.session_keys.entry(address).or_default();
        let authentication_key = session_key.authentication_key();
        let local_session_key = inner_map.get_mut(&authentication_key).ok_or_else(||{
            anyhow::Error::new(KanariError::KeyConversionError(format!("Cannot find session key for address:[{address}] and authentication_key:[{authentication_key}]", address = address, authentication_key = authentication_key)))
        })?;
        local_session_key.session_key = Some(session_key);
        Ok(())
    }

    fn sign_transaction_via_session_key(
        &self,
        address: &KanariAddress,
        msg: KanariTransactionData,
        authentication_key: &AuthenticationKey,
        password: Option<String>,
    ) -> Result<KanariTransaction, anyhow::Error> {
        let local_session_key = self
            .session_keys
            .get(address)
            .ok_or_else(|| {
                signature::Error::from_source(format!(
                    "Cannot find SessionKey for address: [{address}]"
                ))
            })?
            .get(authentication_key)
            .ok_or_else(|| {
                signature::Error::from_source(format!(
                    "Cannot find SessionKey for authentication_key: [{authentication_key}]"
                ))
            })?;

        let kp: KanariKeyPair = local_session_key
            .private_key
            .decrypt_with_type(password)
            .map_err(signature::Error::from_source)?;

        let auth = authenticator::Authenticator::session(&kp, &msg);
        Ok(KanariTransaction::new(msg, auth))
    }

    fn get_session_key(
        &self,
        address: &KanariAddress,
        authentication_key: &AuthenticationKey,
        password: Option<String>,
    ) -> Result<Option<KanariKeyPair>, anyhow::Error> {
        Ok(self.session_keys.get(address).ok_or_else(|| {
            anyhow::Error::new(KanariError::KeyConversionError(format!("Cannot find session key for address:[{address}] and authentication_key:[{authentication_key}]", address = address, authentication_key = authentication_key)))
        })?
        .get(authentication_key)
        .map(|local_session_key| local_session_key.private_key.decrypt_with_type(password).map_err(signature::Error::from_source)).transpose()?)
    }

    fn addresses(&self) -> Vec<KanariAddress> {
        // Create an empty Vec to store the addresses.
        let mut addresses = Vec::with_capacity(self.keys.len() + self.session_keys.len());

        // Iterate over the `keys` and `session_keys` BTreeMaps.
        for key in self.keys.keys() {
            addresses.push(*key);
        }

        for key in self.session_keys.keys() {
            addresses.push(*key);
        }

        addresses
    }

    fn set_password_hash_with_indicator(
        &mut self,
        password_hash: String,
        is_password_empty: bool,
    ) -> Result<(), anyhow::Error> {
        self.password_hash = Some(password_hash);
        self.is_password_empty = is_password_empty;
        Ok(())
    }

    fn get_password_hash(&self) -> String {
        self.password_hash.clone().unwrap_or_default()
    }

    fn get_if_password_is_empty(&self) -> bool {
        self.is_password_empty
    }

    fn get_mnemonic(&self, password: Option<String>) -> Result<MnemonicResult, anyhow::Error> {
        match &self.mnemonic {
            Some(mnemonic_data) => {
                let mnemonic_phrase = mnemonic_data.mnemonic_phrase_encryption.decrypt(password)?;

                let mnemonic_phrase = String::from_utf8(mnemonic_phrase)
                    .map_err(|e| anyhow::anyhow!("Parse mnemonic phrase error:{}", e))?;
                Ok(MnemonicResult {
                    mnemonic_phrase,
                    mnemonic_data: mnemonic_data.clone(),
                })
            }
            None => Err(anyhow::Error::new(KanariError::KeyConversionError(
                "Cannot find mnemonic data, please init the keystore first".to_string(),
            ))),
        }
    }
}
