// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::{
    addresses::MOVE_STD_ADDRESS,
    state::{MoveStructState, MoveStructType},
};
use move_core_types::{
    account_address::AccountAddress,
    ident_str,
    identifier::{IdentStr, Identifier},
    value::{MoveStructLayout, MoveTypeLayout},
};
use move_resource_viewer::{AnnotatedMoveStruct, AnnotatedMoveValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Eq, PartialEq, Clone, PartialOrd, Ord, Hash, Default, JsonSchema)]
pub struct MoveString {
    bytes: Vec<u8>,
}

impl MoveString {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl MoveString {
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes).expect("MoveString is not utf8")
    }
}

impl std::fmt::Debug for MoveString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::fmt::Display for MoveString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(&self.bytes) {
            Ok(s) => {
                write!(f, "{}", s)
            }
            Err(_) => {
                write!(f, "{:?}", &self.bytes)
            }
        }
    }
}

impl From<String> for MoveString {
    fn from(s: String) -> Self {
        MoveString {
            bytes: s.into_bytes(),
        }
    }
}

impl From<&str> for MoveString {
    fn from(s: &str) -> Self {
        MoveString {
            bytes: s.as_bytes().to_vec(),
        }
    }
}

impl From<&IdentStr> for MoveString {
    fn from(s: &IdentStr) -> Self {
        MoveString {
            bytes: s.as_str().as_bytes().to_vec(),
        }
    }
}

impl From<Identifier> for MoveString {
    fn from(s: Identifier) -> Self {
        MoveString {
            bytes: s.as_str().as_bytes().to_vec(),
        }
    }
}

impl FromStr for MoveString {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(MoveString {
            bytes: s.as_bytes().to_vec(),
        })
    }
}

impl MoveStructType for MoveString {
    const ADDRESS: AccountAddress = MOVE_STD_ADDRESS;
    const MODULE_NAME: &'static IdentStr = ident_str!("string");
    const STRUCT_NAME: &'static IdentStr = ident_str!("String");
}

impl MoveStructState for MoveString {
    fn struct_layout() -> MoveStructLayout {
        MoveStructLayout::new(vec![MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8))])
    }
}

impl Serialize for MoveString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(self.to_string().as_str())
        } else {
            serializer.serialize_bytes(&self.bytes)
        }
    }
}

impl<'de> Deserialize<'de> for MoveString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            Ok(MoveString::from_str(s.as_str()).map_err(serde::de::Error::custom)?)
        } else {
            //TODO should we check utf8 here?
            let bytes = Vec::<u8>::deserialize(deserializer)?;
            Ok(MoveString { bytes })
        }
    }
}

impl TryFrom<AnnotatedMoveStruct> for MoveString {
    type Error = anyhow::Error;

    fn try_from(value: AnnotatedMoveStruct) -> Result<Self, Self::Error> {
        MoveString::try_from(&value)
    }
}

impl TryFrom<&AnnotatedMoveStruct> for MoveString {
    type Error = anyhow::Error;

    fn try_from(annotated_move_struct: &AnnotatedMoveStruct) -> Result<Self, Self::Error> {
        let (field_name, field_value) = annotated_move_struct
            .value
            .first()
            .ok_or_else(|| anyhow::anyhow!("Invalid MoveString"))?;
        debug_assert!(field_name.as_str() == "bytes");
        let bytes = match field_value {
            AnnotatedMoveValue::Bytes(bytes) => bytes,
            _ => return Err(anyhow::anyhow!("Invalid MoveString")),
        };
        Ok(MoveString {
            bytes: bytes.clone(),
        })
    }
}

impl TryFrom<MoveString> for Identifier {
    type Error = anyhow::Error;

    fn try_from(value: MoveString) -> Result<Self, Self::Error> {
        Identifier::new(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_utf8() {
        let move_str = MoveString::from_str("你好").unwrap();
        println!("{}", hex::encode(&move_str.bytes));
        println!("{}", move_str);
    }
}
