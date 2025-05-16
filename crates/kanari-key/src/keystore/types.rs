// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use kanari_types::{
    address::{BitcoinAddress, KanariAddress},
    crypto::PublicKey,
    framework::session_key::SessionKey,
    key_struct::EncryptionData,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct LocalSessionKey {
    pub session_key: Option<SessionKey>,
    pub private_key: EncryptionData,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LocalAccount {
    pub address: KanariAddress,
    pub bitcoin_address: BitcoinAddress,
    pub nostr_bech32_public_key: String,
    pub public_key: PublicKey,
    pub has_session_key: bool,
}
