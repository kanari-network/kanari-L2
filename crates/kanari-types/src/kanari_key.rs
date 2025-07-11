// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use bech32::{Hrp, decode};
use bitcoin::{
    hex::{Case, DisplayHex},
    secp256k1::SecretKey,
};
use once_cell::sync::Lazy;

use crate::{crypto::SignatureScheme, error::KanariError};

pub static KANARI_SECRET_KEY_HRP: Lazy<Hrp> =
    Lazy::new(|| Hrp::parse("kanarisecretkey").expect("kanarisecretkey is a valid HRP"));

/// Kanari Key length in bech32 string length: 14 hrp + 60 data
pub const LENGTH_SK_BECH32: usize = 74;

// Parsed Kanari Key, either a bech32 encoded private key or a raw material key
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct ParsedSecretKey(SecretKey);

impl ParsedSecretKey {
    pub fn into_inner(self) -> SecretKey {
        self.0
    }

    pub fn parse(s: &str) -> anyhow::Result<Self, anyhow::Error> {
        if s.starts_with(KANARI_SECRET_KEY_HRP.as_str()) && s.len() == LENGTH_SK_BECH32 {
            let (hrp, data) = decode(s)?;
            if hrp != *KANARI_SECRET_KEY_HRP {
                return Err(anyhow::Error::new(KanariError::CommandArgumentError(
                    format!("Hrp [{:?}] check failed", hrp.to_string()),
                )));
            };
            if data.len() != 33 {
                return Err(anyhow::Error::new(KanariError::CommandArgumentError(
                    format!(
                        "Private key [{:?}] length check failed",
                        data.to_hex_string(Case::Lower)
                    ),
                )));
            };
            if data[0] != SignatureScheme::Secp256k1.flag() {
                return Err(anyhow::Error::new(KanariError::CommandArgumentError(
                    format!("Flag [{:?}] check failed", data[0]),
                )));
            };
            Ok(Self(SecretKey::from_slice(&data[1..])?))
        } else {
            let s = s.strip_prefix("0x").unwrap_or(s);
            match hex::decode(s) {
                Ok(data) => match SecretKey::from_slice(&data) {
                    Ok(a) => Ok(Self(a)),
                    Err(_) => Err(anyhow::Error::new(KanariError::CommandArgumentError(
                        "Parse from a raw material key failed".to_owned(),
                    ))),
                },
                Err(_) => Err(anyhow::Error::new(KanariError::CommandArgumentError(
                    "Secret hex decode failed".to_owned(),
                ))),
            }
        }
    }
}
