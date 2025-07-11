// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_key::keystore::memory_keystore::InMemKeystore;
use kanari_types::framework::auth_validator::{AuthValidatorCaller, BuiltinAuthValidator};
use kanari_types::framework::empty::Empty;
use kanari_types::transaction::kanari::KanariTransactionData;
use move_core_types::vm_status::VMStatus;
use moveos_types::transaction::{MoveAction, MoveOSTransaction};

use crate::binding_test;

#[tokio::test]
async fn test_validate() {
    let binding_test = binding_test::RustBindingTest::new().unwrap();
    let root = binding_test.root().clone();

    let auth_validator = BuiltinAuthValidator::Bitcoin.auth_validator();
    let validator_caller = AuthValidatorCaller::new(&binding_test, auth_validator);

    let keystore = InMemKeystore::new_insecure_for_tests(1);
    let sender = keystore.addresses()[0];
    let sequence_number = 0;
    let action = MoveAction::new_function_call(Empty::empty_function_id(), vec![], vec![]);
    let tx_data = KanariTransactionData::new_for_test(sender, sequence_number, action);
    let tx = keystore.sign_transaction(&sender, tx_data, None).unwrap();
    let auth_info = tx.authenticator_info();

    let move_tx: MoveOSTransaction = tx.into_moveos_transaction(root);

    let result = validator_caller
        .validate(&move_tx.ctx, auth_info.authenticator.payload)
        .unwrap();
    assert_eq!(result.vm_status, VMStatus::Executed);
}
