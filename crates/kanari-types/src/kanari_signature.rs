// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::crypto::Signature;
use fastcrypto::traits::ToFromBytes;
use serde::{Deserialize, Serialize};

// Parsed Kanari Signature, either Ed25519KanariSignature or Secp256k1KanariSignature
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSignature(Signature);

impl ParsedSignature {
    pub fn into_inner(self) -> Signature {
        self.0
    }

    pub fn from_signature(signature: Signature) -> Self {
        Self(signature)
    }

    pub fn parse(s: &str) -> anyhow::Result<Self, anyhow::Error> {
        let signature_bytes = hex::decode(s)?;
        Ok(Self::from_signature(Signature::from_bytes(
            &signature_bytes,
        )?))
    }
}
