// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::binding_test;
use kanari_types::{framework::chain_id::ChainID, kanari_network::BuiltinChainID};
use moveos_types::{module_binding::MoveFunctionCaller, state_resolver::StateResolver};

#[tokio::test]
async fn test_chain_id() {
    let _ = tracing_subscriber::fmt::try_init();
    let binding_test = binding_test::RustBindingTest::new().unwrap();
    let resolver = binding_test.resolver();
    let chain_id = resolver.get_object(&ChainID::chain_id_object_id()).unwrap();
    assert!(chain_id.is_some());
    let chain_id_module =
        binding_test.as_module_binding::<kanari_types::framework::chain_id::ChainIDModule>();
    let chain_id = chain_id_module.chain_id().unwrap();
    assert_eq!(chain_id, BuiltinChainID::Local.chain_id().id());
}
