// Copyright (c) kanari Network
// SPDX-License-Identifier: Apache-2.0

module kanari_framework::core_addresses {
    use std::signer;

    /// The address/account did not correspond to the genesis address
    const ErrorNotGenesisAddress: u64 = 1;
    /// The address/account did not correspond to the core framework address
    const ErrorNotkanariFrameworkAddress: u64 = 2;

    public fun assert_kanari_genesis(account: &signer) {
        assert_kanari_genesis_address(signer::address_of(account))
    }

    public fun assert_kanari_genesis_address(addr: address) {
        assert!(is_kanari_genesis_address(addr), ErrorNotGenesisAddress)
    }

    public fun is_kanari_genesis_address(addr: address): bool {
        addr == genesis_address()
    }

    public fun assert_kanari_framework(account: &signer) {
        assert!(
            is_kanari_framework_address(signer::address_of(account)),
            ErrorNotkanariFrameworkAddress,
        )
    }

    /// Return true if `addr` is 0x3.
    public fun is_kanari_framework_address(addr: address): bool {
        addr == @kanari_framework
    }

    /// The address of the genesis
    public fun genesis_address(): address {
        @kanari_framework
    }
}
