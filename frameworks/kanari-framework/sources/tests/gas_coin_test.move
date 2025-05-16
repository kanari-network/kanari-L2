// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

#[test_only]
/// This test module is used to test the gas coin
module kanari_framework::gas_coin_test{

    use kanari_framework::account as account_entry;
    use kanari_framework::coin;
    use kanari_framework::kari::{Self, KARI};

    #[test]
    fun test_gas_coin_init(){
        kanari_framework::genesis::init_for_test();
        assert!(coin::is_registered<KARI>(), 1000);

    }

    #[test]
    fun test_gas_coin_mint(){
        kanari_framework::genesis::init_for_test();
        let gas_coin = kari::mint_for_test(1000u256);
        kari::burn(gas_coin);

    }

    #[test(user = @0x42)]
    fun test_faucet(user: address){
        kanari_framework::genesis::init_for_test();
        account_entry::create_account_for_testing(user);
        let init_gas = 9999u256;
        kari::faucet_for_test(user, init_gas);
        std::debug::print(&kari::balance(user));
        assert!(kari::balance(user) == init_gas, 1000);
        
    }
}
