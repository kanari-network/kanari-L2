// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::STATIC_FRAMEWORK_DIR;
use framework_types::addresses::*;
use move_core_types::{account_address::AccountAddress, errmap::ErrorMapping};
use once_cell::sync::Lazy;
use std::collections::BTreeMap;

pub static ERROR_DESCRIPTIONS: Lazy<BTreeMap<AccountAddress, ErrorMapping>> = Lazy::new(|| {
    let mut error_descriptions = BTreeMap::new();

    let move_stdlib_err: ErrorMapping = bcs::from_bytes(
        STATIC_FRAMEWORK_DIR
            .get_file("latest/move_stdlib_error_description.errmap")
            .expect("Failed to find move_stdlib_error_description.errmap in STATIC_FRAMEWORK_DIR")
            .contents(),
    )
    .expect("Failed to deserialize move_stdlib_error_description.errmap");
    error_descriptions.insert(MOVE_STD_ADDRESS, move_stdlib_err);

    let moveos_std_err: ErrorMapping = bcs::from_bytes(
        STATIC_FRAMEWORK_DIR
            .get_file("latest/moveos_stdlib_error_description.errmap")
            .expect("Failed to find moveos_stdlib_error_description.errmap in STATIC_FRAMEWORK_DIR")
            .contents(),
    )
    .expect("Failed to deserialize moveos_stdlib_error_description.errmap");
    error_descriptions.insert(MOVEOS_STD_ADDRESS, moveos_std_err);

    let kanari_framework_err: ErrorMapping = bcs::from_bytes(
        STATIC_FRAMEWORK_DIR
            .get_file("latest/kanari_framework_error_description.errmap")
            .expect(
                "Failed to find kanari_framework_error_description.errmap in STATIC_FRAMEWORK_DIR",
            )
            .contents(),
    )
    .expect("Failed to deserialize kanari_framework_error_description.errmap");

    error_descriptions.insert(KANARI_FRAMEWORK_ADDRESS, kanari_framework_err);

    let bitcoin_move_err: ErrorMapping = bcs::from_bytes(
        STATIC_FRAMEWORK_DIR
            .get_file("latest/bitcoin_move_error_description.errmap")
            .expect("Failed to find bitcoin_move_error_description.errmap in STATIC_FRAMEWORK_DIR")
            .contents(),
    )
    .expect("Failed to deserialize bitcoin_move_error_description.errmap");

    error_descriptions.insert(BITCOIN_MOVE_ADDRESS, bitcoin_move_err);

    let kanari_nursery_err: ErrorMapping = bcs::from_bytes(
        STATIC_FRAMEWORK_DIR
            .get_file("latest/kanari_nursery_error_description.errmap")
            .expect(
                "Failed to find kanari_nursery_error_description.errmap in STATIC_FRAMEWORK_DIR",
            )
            .contents(),
    )
    .expect("Failed to deserialize kanari_nursery_error_description.errmap");

    error_descriptions.insert(KANARI_NURSERY_ADDRESS, kanari_nursery_err);

    error_descriptions
});

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_error_descriptions() {
        let error_descriptions = ERROR_DESCRIPTIONS.clone();
        let error_mapping = error_descriptions.get(&MOVEOS_STD_ADDRESS).unwrap();
        //println!("{:?}",error_mapping.module_error_maps);
        let description = error_mapping.get_explanation("0x2::object", 1);
        //println!("{:?}",description);
        assert!(description.is_some());
        let description = description.unwrap();
        assert_eq!(description.code_name.as_str(), "ErrorAlreadyExists");
    }
}
