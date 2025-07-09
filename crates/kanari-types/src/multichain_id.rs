// Copyright (c) K Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, bail, format_err};
use move_core_types::language_storage::TypeTag;
use move_core_types::value::MoveTypeLayout;
use moveos_types::state::{MoveState, MoveType};
#[cfg(any(test, feature = "fuzzing"))]
use proptest_derive::Arbitrary;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

pub const BITCOIN: u64 = 0;
pub const ETHER: u64 = 60;
pub const SUI: u64 = 784;
pub const NOSTR: u64 = 1237;
pub const KANARI: u64 = 20230101; // place holder for slip-0044 needs to replace later

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Serialize,
    Hash,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    JsonSchema,
)]
#[cfg_attr(any(test, feature = "fuzzing"), derive(Arbitrary))]
pub struct MultiChainID {
    id: u64,
}

impl MultiChainID {
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    pub fn id(self) -> u64 {
        self.id
    }

    pub fn is_ethereum(self) -> bool {
        self.id == ETHER
    }

    pub fn is_sui(self) -> bool {
        self.id == SUI
    }

    pub fn is_bitcoin(self) -> bool {
        self.id == BITCOIN
    }

    pub fn is_nostr(self) -> bool {
        self.id == NOSTR
    }
}

impl Display for MultiChainID {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl FromStr for MultiChainID {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id: u64 = s.parse()?;
        Ok(MultiChainID::new(id))
    }
}

impl From<u64> for MultiChainID {
    fn from(id: u64) -> Self {
        Self::new(id)
    }
}

#[allow(clippy::from_over_into)]
impl Into<u64> for MultiChainID {
    fn into(self) -> u64 {
        self.id
    }
}

// BuiltinMultiChainID is following coin standard: https://github.com/satoshilabs/slips/blob/master/slip-0044.md
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[repr(u64)]
#[cfg_attr(any(test, feature = "fuzzing"), derive(Arbitrary))]
pub enum KanariMultiChainID {
    Bitcoin = BITCOIN,
    Ether = ETHER,
    Nostr = NOSTR,
    Kanari = KANARI,
}

impl Display for KanariMultiChainID {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            KanariMultiChainID::Bitcoin => write!(f, "bitcoin"),
            KanariMultiChainID::Ether => write!(f, "ether"),
            KanariMultiChainID::Nostr => write!(f, "nostr"),
            KanariMultiChainID::Kanari => write!(f, "kanari"),
        }
    }
}

impl From<KanariMultiChainID> for u64 {
    fn from(multichain_id: KanariMultiChainID) -> Self {
        multichain_id as u64
    }
}

impl TryFrom<u64> for KanariMultiChainID {
    type Error = anyhow::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            BITCOIN => Ok(KanariMultiChainID::Bitcoin),
            ETHER => Ok(KanariMultiChainID::Ether),
            NOSTR => Ok(KanariMultiChainID::Nostr),
            KANARI => Ok(KanariMultiChainID::Kanari),
            _ => Err(anyhow::anyhow!("multichain id {} is invalid", value)),
        }
    }
}

impl FromStr for KanariMultiChainID {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bitcoin" => Ok(KanariMultiChainID::Bitcoin),
            "ether" => Ok(KanariMultiChainID::Ether),
            "nostr" => Ok(KanariMultiChainID::Nostr),
            "kanari" => Ok(KanariMultiChainID::Kanari),
            s => Err(format_err!("Unknown multichain: {}", s)),
        }
    }
}

impl TryFrom<MultiChainID> for KanariMultiChainID {
    type Error = anyhow::Error;
    fn try_from(multichain_id: MultiChainID) -> Result<Self, Self::Error> {
        Ok(match multichain_id.id() {
            BITCOIN => Self::Bitcoin,
            ETHER => Self::Ether,
            NOSTR => Self::Nostr,
            KANARI => Self::Kanari,
            id => bail!("{} is not a builtin multichain id", id),
        })
    }
}

impl MoveType for KanariMultiChainID {
    fn type_tag() -> TypeTag {
        TypeTag::U64
    }
}

impl MoveState for KanariMultiChainID {
    fn type_layout() -> MoveTypeLayout {
        MoveTypeLayout::U64
    }

    fn to_runtime_value(&self) -> move_vm_types::values::Value {
        move_vm_types::values::Value::u64(*self as u64)
    }

    fn from_runtime_value(value: move_vm_types::values::Value) -> anyhow::Result<Self> {
        KanariMultiChainID::try_from(value.value_as::<u64>()?)
    }
}

impl KanariMultiChainID {
    pub fn multichain_name(self) -> String {
        self.to_string()
    }

    pub fn multichain_id(self) -> MultiChainID {
        MultiChainID::new(self.into())
    }

    pub fn is_bitcoin(self) -> bool {
        matches!(self, KanariMultiChainID::Bitcoin)
    }

    pub fn is_ethereum(self) -> bool {
        matches!(self, KanariMultiChainID::Ether)
    }

    pub fn is_nostr(self) -> bool {
        matches!(self, KanariMultiChainID::Nostr)
    }

    pub fn is_kanari(self) -> bool {
        matches!(self, KanariMultiChainID::Kanari)
    }

    pub fn multichain_ids() -> Vec<KanariMultiChainID> {
        vec![
            KanariMultiChainID::Bitcoin,
            KanariMultiChainID::Ether,
            KanariMultiChainID::Nostr,
            KanariMultiChainID::Kanari,
        ]
    }
}
