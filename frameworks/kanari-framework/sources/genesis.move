// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

module kanari_framework::genesis {

    use std::option;
    use std::vector;
    use moveos_std::signer;
    use moveos_std::tx_context;
    use moveos_std::module_store;
    use moveos_std::core_addresses;
    use kanari_framework::account;
    use kanari_framework::auth_validator_registry;
    use kanari_framework::builtin_validators;
    use kanari_framework::chain_id;
    use kanari_framework::coin;
    use kanari_framework::account_coin_store;
    use kanari_framework::kari;
    use kanari_framework::transaction_fee;
    use kanari_framework::address_mapping;
    use kanari_framework::onchain_config;
    use kanari_framework::bitcoin_address::{Self, BitcoinAddress};
    use kanari_framework::did;

    const ErrorGenesisInit: u64 = 1;

    const GENESIS_INIT_GAS_AMOUNT: u256 = 100000000_00000000u256;

    /// GenesisContext is a genesis init parameters in the TxContext.
    struct GenesisContext has copy,store,drop{
        chain_id: u64,
        /// Sequencer account
        sequencer: BitcoinAddress, 
        /// kanari DAO multisign account
        kanari_dao: BitcoinAddress, 
    }

    fun init(){
        // create all system accounts
        let system_addresses = core_addresses::list_system_reserved_addresses();
        vector::for_each(system_addresses, |addr| {
            let _ = account::create_system_account(addr);
        });

        let genesis_account = &signer::module_signer<GenesisContext>();
        let genesis_context_option = tx_context::get_attribute<GenesisContext>();
        assert!(option::is_some(&genesis_context_option), ErrorGenesisInit);
        let genesis_context = option::extract(&mut genesis_context_option);
        chain_id::genesis_init(genesis_account, genesis_context.chain_id);
        auth_validator_registry::genesis_init(genesis_account);
        builtin_validators::genesis_init(genesis_account);
        coin::genesis_init(genesis_account);
        account_coin_store::genesis_init(genesis_account);
        kari::genesis_init(genesis_account);
        transaction_fee::genesis_init(genesis_account);
        address_mapping::genesis_init(genesis_account);
        let sequencer_addr = bitcoin_address::to_kanari_address(&genesis_context.sequencer);
        
        // Some test cases use framework account as sequencer, it may already exist
        if(!moveos_std::account::exists_at(sequencer_addr)){
            account::create_account(sequencer_addr);
            address_mapping::bind_bitcoin_address_internal(sequencer_addr, genesis_context.sequencer);
        };
        let kanari_dao_address = bitcoin_address::to_kanari_address(&genesis_context.kanari_dao);

        onchain_config::genesis_init(genesis_account, sequencer_addr, kanari_dao_address);

        // issue framework packages upgrade cap to the kanari dao
        let system_addresses = core_addresses::list_system_reserved_addresses();
        vector::for_each(system_addresses, |addr| {
            module_store::issue_upgrade_cap_by_system(genesis_account, addr, kanari_dao_address);
        });
        
        // give initial gas to the kanari dao
        kari::faucet(kanari_dao_address, GENESIS_INIT_GAS_AMOUNT);

        // give initial gas to the sequencer if it's not mainnet
        if(!chain_id::is_main()){
            kari::faucet(sequencer_addr, GENESIS_INIT_GAS_AMOUNT);
        };

        did::genesis_init();
    }

    #[test_only]
    use moveos_std::genesis;

    #[test_only]
    /// init the genesis context for test
    public fun init_for_test(){
        let genesis_account = moveos_std::signer::module_signer<GenesisContext>();
        let sequencer = bitcoin_address::from_string(&std::string::utf8(b"bc1pxup9p7um3t5knqn0yxfrq5d0mgul9ts993j32tsfxn68qa4pl3nq2qhh2e"));
        tx_context::add_attribute_via_system(&genesis_account, GenesisContext{chain_id: 3, sequencer, kanari_dao: bitcoin_address::from_string(&std::string::utf8(b"bc1pevdrc8yqmgd94h2mpz9st0u77htmx935hzck3ruwsvcf4w7wrnqqd0yvze"))});
        genesis::init_for_test();
        init();
    }
}