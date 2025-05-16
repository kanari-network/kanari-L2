// Copyright (c) KanariNetwork
// SPDX-License-Identifier: Apache-2.0

module kanari_nursery::genesis {
    use kanari_nursery::ethereum;
    use kanari_nursery::tick_info;
    use kanari_nursery::inscribe_factory;

    const ErrorInvalidChainId: u64 = 1;

    struct GenesisContext has copy,store,drop{
    }

    fun init(genesis_account: &signer){
        ethereum::genesis_init(genesis_account);
        tick_info::genesis_init();
        inscribe_factory::genesis_init();
    }

    #[test_only]
    /// init the genesis context for test
    public fun init_for_test(){
        kanari_nursery::genesis::init_for_test();
        let genesis_account = moveos_std::signer::module_signer<GenesisContext>();
        init(&genesis_account);
    }
}