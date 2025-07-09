// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::jsonrpc_types::StrView;
use anyhow::Result;
use bitcoin::XOnlyPublicKey;
use kanari_types::{
    address::{BitcoinAddress, KanariAddress, NostrPublicKey},
    bitcoin::network::Network,
    to_bech32::FromBech32,
};
use move_core_types::account_address::AccountAddress;
use std::str::FromStr;

pub type BitcoinAddressView = StrView<BitcoinAddress>;

impl std::fmt::Display for BitcoinAddressView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //The Display Bitcoin address as a hexadecimal string
        write!(f, "{}", self.0)
    }
}

impl FromStr for BitcoinAddressView {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(StrView(BitcoinAddress::from_str(s)?))
    }
}

impl From<BitcoinAddressView> for BitcoinAddress {
    fn from(value: BitcoinAddressView) -> Self {
        value.0
    }
}

pub type KanariAddressView = StrView<KanariAddress>;

impl std::fmt::Display for KanariAddressView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for KanariAddressView {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(StrView(KanariAddress::from_str(s)?))
    }
}

impl From<KanariAddressView> for KanariAddress {
    fn from(value: KanariAddressView) -> Self {
        value.0
    }
}

impl From<AccountAddress> for KanariAddressView {
    fn from(value: AccountAddress) -> Self {
        StrView(KanariAddress::from(value))
    }
}

impl From<KanariAddressView> for AccountAddress {
    fn from(value: KanariAddressView) -> Self {
        value.0.into()
    }
}

//TODO directly use UnitedAddress and remove UnitedAddressView
#[derive(Debug, Clone)]
pub struct UnitedAddress {
    pub kanari_address: KanariAddress,
    pub bitcoin_address: Option<BitcoinAddress>,
    pub nostr_public_key: Option<NostrPublicKey>,
}

pub type UnitedAddressView = StrView<UnitedAddress>;

impl std::fmt::Display for UnitedAddressView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(bitcoin_address) = &self.0.bitcoin_address {
            return write!(f, "{}", bitcoin_address);
        }
        if let Some(nostr_public_key) = &self.0.nostr_public_key {
            return write!(f, "{}", nostr_public_key);
        }
        write!(f, "{}", self.0.kanari_address)
    }
}

impl FromStr for UnitedAddressView {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        //TODO use the prefix to determine the type of address
        match KanariAddress::from_str(s) {
            Ok(kanari_address) => Ok(StrView(UnitedAddress {
                kanari_address,
                bitcoin_address: None,
                nostr_public_key: None,
            })),
            Err(_) => match XOnlyPublicKey::from_bech32(s) {
                Ok(x_only_pk) => {
                    let nostr_public_key = NostrPublicKey::new(x_only_pk);
                    let bitcoin_address =
                        nostr_public_key.to_bitcoin_address(Network::Bitcoin.to_num())?;
                    let kanari_address = bitcoin_address.to_kanari_address();
                    Ok(StrView(UnitedAddress {
                        kanari_address,
                        bitcoin_address: Some(bitcoin_address),
                        nostr_public_key: Some(nostr_public_key),
                    }))
                }
                Err(_) => {
                    let bitcoin_address = BitcoinAddress::from_str(s)?;
                    let kanari_address = bitcoin_address.to_kanari_address();
                    Ok(StrView(UnitedAddress {
                        kanari_address,
                        bitcoin_address: Some(bitcoin_address),
                        nostr_public_key: None,
                    }))
                }
            },
        }
    }
}

impl From<UnitedAddressView> for KanariAddress {
    fn from(value: UnitedAddressView) -> Self {
        value.0.kanari_address
    }
}

impl From<UnitedAddressView> for AccountAddress {
    fn from(value: UnitedAddressView) -> Self {
        value.0.kanari_address.into()
    }
}

impl From<KanariAddressView> for UnitedAddressView {
    fn from(value: KanariAddressView) -> Self {
        StrView(UnitedAddress {
            kanari_address: value.into(),
            bitcoin_address: None,
            nostr_public_key: None,
        })
    }
}

impl TryFrom<UnitedAddressView> for BitcoinAddress {
    type Error = anyhow::Error;

    fn try_from(value: UnitedAddressView) -> Result<Self, Self::Error> {
        match value.0.bitcoin_address {
            Some(bitcoin_address) => Ok(bitcoin_address),
            None => Err(anyhow::anyhow!("No Bitcoin address found")),
        }
    }
}

impl TryFrom<UnitedAddressView> for NostrPublicKey {
    type Error = anyhow::Error;

    fn try_from(value: UnitedAddressView) -> Result<Self, Self::Error> {
        match value.0.nostr_public_key {
            Some(nostr_public_key) => Ok(nostr_public_key),
            None => Err(anyhow::anyhow!("No Nostr public key found")),
        }
    }
}

impl From<BitcoinAddress> for UnitedAddressView {
    fn from(value: BitcoinAddress) -> Self {
        StrView(UnitedAddress {
            kanari_address: value.to_kanari_address(),
            bitcoin_address: Some(value),
            nostr_public_key: None,
        })
    }
}

impl From<KanariAddress> for UnitedAddressView {
    fn from(value: KanariAddress) -> Self {
        StrView(UnitedAddress {
            kanari_address: value,
            bitcoin_address: None,
            nostr_public_key: None,
        })
    }
}

impl From<bitcoin::Address> for UnitedAddressView {
    fn from(value: bitcoin::Address) -> Self {
        let value = BitcoinAddress::from(value);
        StrView(UnitedAddress {
            kanari_address: value.to_kanari_address(),
            bitcoin_address: Some(value),
            nostr_public_key: None,
        })
    }
}
