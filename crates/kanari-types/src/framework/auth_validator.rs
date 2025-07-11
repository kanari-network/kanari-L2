// Copyright (c) kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::address::{BitcoinAddress, KanariSupportedAddress};
use crate::addresses::KANARI_FRAMEWORK_ADDRESS;
use crate::error::KanariError;
use anyhow::Result;
use clap::ValueEnum;
use framework_types::addresses::KANARI_NURSERY_ADDRESS;
use move_core_types::value::MoveValue;
use move_core_types::{
    account_address::AccountAddress, ident_str, identifier::IdentStr, language_storage::ModuleId,
};
use moveos_types::function_return_value::DecodedFunctionResult;
use moveos_types::move_std::option::MoveOption;
use moveos_types::state::MoveState;
use moveos_types::{
    module_binding::MoveFunctionCaller,
    move_std::string::MoveString,
    move_types::FunctionId,
    moveos_std::tx_context::TxContext,
    state::{MoveStructState, MoveStructType},
    transaction::FunctionCall,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use strum_macros::{Display, EnumString};

pub const MODULE_NAME: &IdentStr = ident_str!("auth_validator");

/// The Authenticator auth validator which has builtin Kanari and Ethereum
#[derive(
    Copy,
    Clone,
    Debug,
    EnumString,
    PartialEq,
    Eq,
    ValueEnum,
    Display,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
)]
#[strum(serialize_all = "lowercase")]
pub enum BuiltinAuthValidator {
    Session,
    Bitcoin,
    BitcoinMultisign,
    Ethereum,
}

impl BuiltinAuthValidator {
    const SESSION_FLAG: u8 = 0x00;
    const BITCOIN_FLAG: u8 = 0x01;
    const BITCOIN_MULTISIGN: u8 = 0x02;
    const ETHEREUM_FLAG: u8 = 0x03;

    pub fn flag(&self) -> u8 {
        match self {
            BuiltinAuthValidator::Session => Self::SESSION_FLAG,
            BuiltinAuthValidator::Bitcoin => Self::BITCOIN_FLAG,
            BuiltinAuthValidator::BitcoinMultisign => Self::BITCOIN_MULTISIGN,
            BuiltinAuthValidator::Ethereum => Self::ETHEREUM_FLAG,
        }
    }

    pub fn from_flag(flag: &str) -> Result<BuiltinAuthValidator, KanariError> {
        let byte_int = flag.parse::<u8>().map_err(|_| {
            KanariError::KeyConversionError("Invalid key auth validator".to_owned())
        })?;
        Self::from_flag_byte(byte_int)
    }

    pub fn from_flag_byte(byte_int: u8) -> Result<BuiltinAuthValidator, KanariError> {
        match byte_int {
            Self::SESSION_FLAG => Ok(BuiltinAuthValidator::Session),
            Self::BITCOIN_FLAG => Ok(BuiltinAuthValidator::Bitcoin),
            Self::BITCOIN_MULTISIGN => Ok(BuiltinAuthValidator::BitcoinMultisign),
            Self::ETHEREUM_FLAG => Ok(BuiltinAuthValidator::Ethereum),
            _ => Err(KanariError::KeyConversionError(
                "Invalid key auth validator".to_owned(),
            )),
        }
    }

    pub fn auth_validator(&self) -> AuthValidator {
        match self {
            BuiltinAuthValidator::Session => AuthValidator {
                id: self.flag().into(),
                module_address: KANARI_FRAMEWORK_ADDRESS,
                module_name: MoveString::from_str("session_validator").expect("Should be valid"),
            },
            BuiltinAuthValidator::Bitcoin => AuthValidator {
                id: self.flag().into(),
                module_address: KANARI_FRAMEWORK_ADDRESS,
                module_name: MoveString::from_str("bitcoin_validator").expect("Should be valid"),
            },
            BuiltinAuthValidator::BitcoinMultisign => AuthValidator {
                id: self.flag().into(),
                module_address: KANARI_NURSERY_ADDRESS,
                module_name: MoveString::from_str("bitcoin_multisign_validator")
                    .expect("Should be valid"),
            },
            BuiltinAuthValidator::Ethereum => AuthValidator {
                id: self.flag().into(),
                module_address: KANARI_NURSERY_ADDRESS,
                module_name: MoveString::from_str("ethereum_validator").expect("Should be valid"),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthValidator {
    pub id: u64,
    pub module_address: AccountAddress,
    pub module_name: MoveString,
}

impl MoveStructType for AuthValidator {
    const ADDRESS: AccountAddress = KANARI_FRAMEWORK_ADDRESS;
    const MODULE_NAME: &'static IdentStr = MODULE_NAME;
    const STRUCT_NAME: &'static IdentStr = ident_str!("AuthValidator");
}

impl MoveStructState for AuthValidator {
    fn struct_layout() -> move_core_types::value::MoveStructLayout {
        move_core_types::value::MoveStructLayout::new(vec![
            move_core_types::value::MoveTypeLayout::U64,
            move_core_types::value::MoveTypeLayout::Address,
            move_core_types::value::MoveTypeLayout::Vector(Box::new(
                move_core_types::value::MoveTypeLayout::U8,
            )),
        ])
    }
}

impl AuthValidator {
    pub const VALIDATE_FUNCTION_NAME: &'static IdentStr = ident_str!("validate");

    pub fn validator_module_id(&self) -> ModuleId {
        ModuleId::new(
            self.module_address,
            self.module_name
                .clone()
                .try_into()
                .expect("Invalid module name"),
        )
    }

    pub fn validator_function_id(&self) -> FunctionId {
        FunctionId::new(
            self.validator_module_id(),
            Self::VALIDATE_FUNCTION_NAME.to_owned(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxValidateResult {
    pub auth_validator_id: u64,
    pub auth_validator: MoveOption<AuthValidator>,
    pub session_key: MoveOption<Vec<u8>>,
    pub bitcoin_address: BitcoinAddress,
}

impl MoveStructType for TxValidateResult {
    const ADDRESS: AccountAddress = KANARI_FRAMEWORK_ADDRESS;
    const MODULE_NAME: &'static IdentStr = MODULE_NAME;
    const STRUCT_NAME: &'static IdentStr = ident_str!("TxValidateResult");
}

impl MoveStructState for TxValidateResult {
    fn struct_layout() -> move_core_types::value::MoveStructLayout {
        move_core_types::value::MoveStructLayout::new(vec![
            move_core_types::value::MoveTypeLayout::U64,
            MoveOption::<AuthValidator>::type_layout(),
            MoveOption::<Vec<u8>>::type_layout(),
            BitcoinAddress::type_layout(),
        ])
    }
}

impl TxValidateResult {
    pub fn new_for_test() -> Self {
        // generate a random bitcoin address for testing
        let bitcoin_address = BitcoinAddress::random();
        Self {
            auth_validator_id: BuiltinAuthValidator::Bitcoin.flag().into(),
            auth_validator: MoveOption::none(),
            session_key: MoveOption::none(),
            bitcoin_address,
        }
    }

    pub fn auth_validator(&self) -> Option<AuthValidator> {
        self.auth_validator.clone().into()
    }

    pub fn session_key(&self) -> Option<Vec<u8>> {
        self.session_key.clone().into()
    }

    pub fn is_validate_via_session_key(&self) -> bool {
        self.session_key().is_some()
    }
}

/// Rust bindings for developer custom auth validator module
/// Because the module is not in KanariFramework, we need to dynamically determine the module id base on the AuthValidator struct
pub struct AuthValidatorCaller<'a> {
    caller: &'a dyn MoveFunctionCaller,
    auth_validator: AuthValidator,
}

impl<'a> AuthValidatorCaller<'a> {
    pub fn new(caller: &'a dyn MoveFunctionCaller, auth_validator: AuthValidator) -> Self {
        Self {
            caller,
            auth_validator,
        }
    }

    pub fn validate(&self, ctx: &TxContext, payload: Vec<u8>) -> Result<DecodedFunctionResult<()>> {
        let auth_validator_call = FunctionCall::new(
            self.auth_validator.validator_function_id(),
            vec![],
            vec![MoveValue::vector_u8(payload).simple_serialize().unwrap()],
        );
        self.caller
            .call_function(ctx, auth_validator_call)?
            .decode(|_values| Ok(()))
    }
}
