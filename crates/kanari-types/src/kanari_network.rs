// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::address::BitcoinAddress;
use crate::bitcoin::genesis::MultisignAccountConfig;
use crate::bitcoin::multisign_account;
use crate::crypto::KanariKeyPair;
use crate::framework::chain_id::ChainID;
use crate::genesis_config::{self, GenesisConfig};
use anyhow::{bail, format_err, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

pub const CHAIN_ID_LOCAL: u64 = 4;
pub const CHAIN_ID_DEV: u64 = 3;
pub const CHAIN_ID_TEST: u64 = 2;
pub const CHAIN_ID_MAIN: u64 = 1;

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(u64)]
pub enum BuiltinChainID {
    /// A temp network just for developer test.
    /// The data is stored in the temporary directory and will be cleared after restarting.
    #[default]
    Local = CHAIN_ID_LOCAL,
    /// A ephemeral network just for developer test.
    Dev = CHAIN_ID_DEV,
    /// Kanari test network.
    /// The data on the chain will be cleaned up periodically.
    Test = CHAIN_ID_TEST,
    /// Kanari main net.
    Main = CHAIN_ID_MAIN,
}

impl Display for BuiltinChainID {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BuiltinChainID::Local => write!(f, "local"),
            BuiltinChainID::Dev => write!(f, "dev"),
            BuiltinChainID::Test => write!(f, "test"),
            BuiltinChainID::Main => write!(f, "main"),
        }
    }
}

impl From<BuiltinChainID> for u64 {
    fn from(chain_id: BuiltinChainID) -> Self {
        chain_id as u64
    }
}

impl TryFrom<u64> for BuiltinChainID {
    type Error = anyhow::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            CHAIN_ID_LOCAL => Ok(BuiltinChainID::Local),
            CHAIN_ID_DEV => Ok(BuiltinChainID::Dev),
            CHAIN_ID_TEST => Ok(BuiltinChainID::Test),
            CHAIN_ID_MAIN => Ok(BuiltinChainID::Main),
            _ => Err(anyhow::anyhow!("chain id {} is invalid", value)),
        }
    }
}

impl FromStr for BuiltinChainID {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase();
        match s.as_str() {
            "local" => Ok(BuiltinChainID::Local),
            "dev" => Ok(BuiltinChainID::Dev),
            "test" => Ok(BuiltinChainID::Test),
            "main" => Ok(BuiltinChainID::Main),
            s => Err(format_err!("Unknown chain: {}", s)),
        }
    }
}

impl TryFrom<ChainID> for BuiltinChainID {
    type Error = anyhow::Error;
    fn try_from(id: ChainID) -> Result<Self, Self::Error> {
        Ok(match id.id() {
            CHAIN_ID_LOCAL => Self::Local,
            CHAIN_ID_DEV => Self::Dev,
            CHAIN_ID_TEST => Self::Test,
            CHAIN_ID_MAIN => Self::Main,
            id => bail!("{} is not a builtin chain id", id),
        })
    }
}

impl From<BuiltinChainID> for ChainID {
    fn from(chain_id: BuiltinChainID) -> Self {
        ChainID::new(chain_id.into())
    }
}

impl BuiltinChainID {
    pub fn chain_name(self) -> String {
        self.to_string().to_lowercase()
    }

    pub fn chain_id(self) -> ChainID {
        ChainID::new(self.into())
    }

    pub fn is_local(self) -> bool {
        matches!(self, BuiltinChainID::Local)
    }

    pub fn is_dev(self) -> bool {
        matches!(self, BuiltinChainID::Dev)
    }

    pub fn is_test(self) -> bool {
        matches!(self, BuiltinChainID::Test)
    }

    pub fn assert_test_or_dev_or_local(self) -> Result<()> {
        if !self.is_test_or_dev_or_local() {
            bail!("Only support test or dev or local network.")
        }
        Ok(())
    }

    pub fn is_test_or_dev_or_local(self) -> bool {
        matches!(
            self,
            BuiltinChainID::Test | BuiltinChainID::Dev | BuiltinChainID::Local
        )
    }

    pub fn is_main(self) -> bool {
        matches!(self, BuiltinChainID::Main)
    }

    pub fn chain_ids() -> Vec<BuiltinChainID> {
        vec![
            BuiltinChainID::Local,
            BuiltinChainID::Dev,
            BuiltinChainID::Test,
            BuiltinChainID::Main,
        ]
    }

    pub fn genesis_config(&self) -> &GenesisConfig {
        match self {
            BuiltinChainID::Local => &genesis_config::G_LOCAL_CONFIG,
            BuiltinChainID::Dev => &genesis_config::G_DEV_CONFIG,
            BuiltinChainID::Test => &genesis_config::G_TEST_CONFIG,
            BuiltinChainID::Main => &genesis_config::G_MAIN_CONFIG,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum KanariChainID {
    Builtin(BuiltinChainID),
    Custom(ChainID),
}

impl Display for KanariChainID {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Builtin(b) => b.to_string(),
            Self::Custom(c) => c.to_string(),
        };
        write!(f, "{}", name)
    }
}

impl From<BuiltinChainID> for KanariChainID {
    fn from(chain_id: BuiltinChainID) -> Self {
        KanariChainID::Builtin(chain_id)
    }
}

impl From<ChainID> for KanariChainID {
    fn from(chain_id: ChainID) -> Self {
        match chain_id.id() {
            CHAIN_ID_LOCAL => KanariChainID::Builtin(BuiltinChainID::Local),
            CHAIN_ID_DEV => KanariChainID::Builtin(BuiltinChainID::Dev),
            CHAIN_ID_TEST => KanariChainID::Builtin(BuiltinChainID::Test),
            CHAIN_ID_MAIN => KanariChainID::Builtin(BuiltinChainID::Main),
            _ => KanariChainID::Custom(chain_id),
        }
    }
}

impl FromStr for KanariChainID {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match BuiltinChainID::from_str(s) {
            Ok(chain_id) => Ok(Self::Builtin(chain_id)),
            Err(_e) => Ok(Self::Custom(ChainID::from_str(s)?)),
        }
    }
}

impl KanariChainID {
    pub fn chain_name(&self) -> String {
        match self {
            Self::Builtin(b) => b.chain_name(),
            Self::Custom(c) => c.to_string(),
        }
    }

    pub fn chain_id(&self) -> ChainID {
        match self {
            Self::Builtin(b) => b.chain_id(),
            Self::Custom(c) => c.clone(),
        }
    }

    pub fn assert_test_or_dev_or_local(&self) -> Result<()> {
        if !self.is_test_or_dev_or_local() {
            bail!("Only support test or dev or local chain_id.")
        }
        Ok(())
    }

    pub fn is_builtin(&self) -> bool {
        self.is_test() || self.is_dev() || self.is_main()
    }

    pub fn is_test_or_dev_or_local(&self) -> bool {
        self.is_test() || self.is_dev() || self.is_local()
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Builtin(BuiltinChainID::Local))
    }

    pub fn is_dev(&self) -> bool {
        matches!(self, Self::Builtin(BuiltinChainID::Dev))
    }

    pub fn is_test(&self) -> bool {
        matches!(self, Self::Builtin(BuiltinChainID::Test))
    }

    pub fn is_main(&self) -> bool {
        matches!(self, Self::Builtin(BuiltinChainID::Main))
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }

    /// Default data dir name of this chain_id
    pub fn dir_name(&self) -> String {
        self.chain_name()
    }
}

impl Default for KanariChainID {
    fn default() -> Self {
        KanariChainID::Builtin(BuiltinChainID::default())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KanariNetwork {
    pub chain_id: ChainID,
    pub genesis_config: GenesisConfig,
}

impl From<BuiltinChainID> for KanariNetwork {
    fn from(chain_id: BuiltinChainID) -> Self {
        KanariNetwork::builtin(chain_id)
    }
}

impl KanariNetwork {
    pub fn new(chain_id: ChainID, genesis_config: GenesisConfig) -> Self {
        Self {
            chain_id,
            genesis_config,
        }
    }

    pub fn builtin(builtin_id: BuiltinChainID) -> Self {
        Self::new(builtin_id.into(), builtin_id.genesis_config().clone())
    }

    pub fn local() -> Self {
        Self::builtin(BuiltinChainID::Local)
    }

    pub fn dev() -> Self {
        Self::builtin(BuiltinChainID::Dev)
    }

    pub fn test() -> Self {
        Self::builtin(BuiltinChainID::Test)
    }

    pub fn main() -> Self {
        Self::builtin(BuiltinChainID::Main)
    }

    /// Mock the genesis account for local dev or unit test.
    pub fn mock_genesis_account(&mut self, kp: &KanariKeyPair) -> Result<BitcoinAddress> {
        let bitcoin_address = kp.public().bitcoin_address()?;
        let bitcoin_public_key = kp.bitcoin_public_key()?;
        let multisign_bitcoin_address =
            multisign_account::generate_multisign_address(1, vec![bitcoin_public_key.to_bytes()])?;
        self.genesis_config.sequencer_account = bitcoin_address;
        self.genesis_config.kanari_dao = MultisignAccountConfig {
            multisign_bitcoin_address: multisign_bitcoin_address.clone(),
            threshold: 1,
            participant_public_keys: vec![bitcoin_public_key.to_bytes()],
        };
        Ok(multisign_bitcoin_address)
    }
}
