// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{account_address::AccountAddress, value::MoveValue};
use moveos_types::{
    moveos_std::object::ObjectID,
    state::{MoveState, ObjectState},
};

#[derive(Debug, Clone)]
pub enum ObjectArg {
    /// The object argument is &mut Object<T>
    Mutref(ObjectState),
    /// The object argument is &Object<T>
    Ref(ObjectState),
    /// The object argument is Object<T>
    Value(ObjectState),
}

impl ObjectArg {
    pub fn object_id(&self) -> &ObjectID {
        match self {
            ObjectArg::Mutref(object) => object.id(),
            ObjectArg::Ref(object) => object.id(),
            ObjectArg::Value(object) => object.id(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedArg {
    Signer { address: AccountAddress },
    Object(ObjectArg),
    ObjectVector(Vec<ObjectArg>),
    Pure { value: Vec<u8> },
}

impl ResolvedArg {
    pub fn signer(address: AccountAddress) -> Self {
        ResolvedArg::Signer { address }
    }

    pub fn object_by_mutref(object: ObjectState) -> Self {
        ResolvedArg::Object(ObjectArg::Mutref(object))
    }

    pub fn object_by_ref(object: ObjectState) -> Self {
        ResolvedArg::Object(ObjectArg::Ref(object))
    }

    pub fn object_by_value(object: ObjectState) -> Self {
        ResolvedArg::Object(ObjectArg::Value(object))
    }

    pub fn pure(value: Vec<u8>) -> Self {
        ResolvedArg::Pure { value }
    }

    pub fn into_serialized_arg(self) -> Vec<u8> {
        match self {
            ResolvedArg::Signer { address } => MoveValue::Signer(address)
                .simple_serialize()
                .expect("serialize signer should success"),
            ResolvedArg::Object(ObjectArg::Mutref(object)) => object.id().to_bytes(),
            ResolvedArg::Object(ObjectArg::Ref(object)) => object.id().to_bytes(),
            ResolvedArg::Object(ObjectArg::Value(object)) => object.id().to_bytes(),
            ResolvedArg::Pure { value } => value,
            ResolvedArg::ObjectVector(value_vector) => {
                let mut vector_object = vec![];
                for object in value_vector.iter() {
                    if let ObjectArg::Value(object) = object {
                        vector_object.push(object.id());
                    }
                }

                bcs::to_bytes(&vector_object).expect("Serialize the ObjectVector should success")
            }
        }
    }
}
