// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::address::{BitcoinAddress, KanariAddress, MultiChainAddress};
use crate::addresses::KANARI_FRAMEWORK_ADDRESS;
use anyhow::{Ok, Result};
use move_core_types::value::MoveTypeLayout;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use moveos_types::moveos_std::object::ObjectID;
use moveos_types::moveos_std::object::{self, ObjectMeta};
use moveos_types::state::{FieldKey, MoveStructState, MoveStructType, ObjectState};
use moveos_types::state_resolver::StateResolver;
use moveos_types::{
    h256::H256,
    module_binding::{ModuleBinding, MoveFunctionCaller},
    move_std::option::MoveOption,
    moveos_std::tx_context::TxContext,
    state::MoveState,
    state::MoveType,
    transaction::FunctionCall,
};
use serde::{Deserialize, Serialize};

pub const MODULE_NAME: &IdentStr = ident_str!("address_mapping");

pub const NAMED_MAPPING_INDEX: u64 = 0;
pub const NAMED_REVERSE_MAPPING_INDEX: u64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultiChainAddressMapping {
    _placeholder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KanariToBitcoinAddressMapping {
    _placeholder: bool,
}

impl MoveStructType for MultiChainAddressMapping {
    const ADDRESS: AccountAddress = KANARI_FRAMEWORK_ADDRESS;
    const MODULE_NAME: &'static IdentStr = MODULE_NAME;
    const STRUCT_NAME: &'static IdentStr = ident_str!("MultiChainAddressMapping");
}

impl MoveStructState for MultiChainAddressMapping {
    fn struct_layout() -> move_core_types::value::MoveStructLayout {
        move_core_types::value::MoveStructLayout::new(vec![MoveTypeLayout::Bool])
    }
}

impl MultiChainAddressMapping {
    pub fn object_id() -> ObjectID {
        object::named_object_id(&Self::struct_tag())
    }
}

impl MoveStructType for KanariToBitcoinAddressMapping {
    const ADDRESS: AccountAddress = KANARI_FRAMEWORK_ADDRESS;
    const MODULE_NAME: &'static IdentStr = MODULE_NAME;
    const STRUCT_NAME: &'static IdentStr = ident_str!("KanariToBitcoinAddressMapping");
}

impl MoveStructState for KanariToBitcoinAddressMapping {
    fn struct_layout() -> move_core_types::value::MoveStructLayout {
        move_core_types::value::MoveStructLayout::new(vec![MoveTypeLayout::Bool])
    }
}

impl KanariToBitcoinAddressMapping {
    pub fn object_id() -> ObjectID {
        object::named_object_id(&Self::struct_tag())
    }

    pub fn genesis() -> ObjectState {
        let id = Self::object_id();
        let mut metadata = ObjectMeta::genesis_meta(id, Self::type_tag());
        metadata.owner = KANARI_FRAMEWORK_ADDRESS;
        ObjectState::new_with_struct(metadata, Self::default())
            .expect("Create KanariToBitcoinAddressMapping Object should success")
    }

    pub fn genesis_with_state_root(state_root: H256, size: u64) -> ObjectState {
        let mut object = Self::genesis();
        object.metadata.state_root = Some(state_root);
        object.metadata.size = size;
        object
    }

    pub fn resolve_bitcoin_address(
        state_resolver: &impl StateResolver,
        address: AccountAddress,
    ) -> Result<Option<BitcoinAddress>> {
        let address_mapping_object_id = KanariToBitcoinAddressMapping::object_id();
        let object_state = state_resolver.get_field(
            &address_mapping_object_id,
            &FieldKey::derive_from_address(&address),
        )?;
        if let Some(object_state) = object_state {
            let df = object_state.value_as_df::<AccountAddress, BitcoinAddress>()?;
            Ok(Some(df.value))
        } else {
            Ok(None)
        }
    }
}

/// Rust bindings for KanariFramework address_mapping module
pub struct AddressMappingModule<'a> {
    caller: &'a dyn MoveFunctionCaller,
}

impl<'a> AddressMappingModule<'a> {
    const RESOLVE_FUNCTION_NAME: &'static IdentStr = ident_str!("resolve");

    pub fn resolve(&self, multichain_address: MultiChainAddress) -> Result<Option<AccountAddress>> {
        if multichain_address.is_kanari_address() {
            let kanari_address: KanariAddress = multichain_address.try_into()?;
            Ok(Some(kanari_address.into()))
        } else if multichain_address.is_bitcoin_address() {
            let bitcoin_address: BitcoinAddress = multichain_address.try_into()?;
            Ok(Some(bitcoin_address.to_kanari_address().into()))
        } else {
            let ctx = TxContext::zero();
            let call = FunctionCall::new(
                Self::function_id(Self::RESOLVE_FUNCTION_NAME),
                vec![],
                vec![multichain_address.to_bytes()],
            );
            let result = self
                .caller
                .call_function(&ctx, call)?
                .into_result()
                .map(|values| {
                    let value = values.first().expect("Expected return value");
                    let result = MoveOption::<AccountAddress>::from_bytes(&value.value)
                        .expect("Expected Option<address>");
                    result.into()
                })?;
            Ok(result)
        }
    }
}

impl<'a> ModuleBinding<'a> for AddressMappingModule<'a> {
    const MODULE_NAME: &'static IdentStr = ident_str!("address_mapping");
    const MODULE_ADDRESS: AccountAddress = KANARI_FRAMEWORK_ADDRESS;

    fn new(caller: &'a impl MoveFunctionCaller) -> Self
    where
        Self: Sized,
    {
        Self { caller }
    }
}
