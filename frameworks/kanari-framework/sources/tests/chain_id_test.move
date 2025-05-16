// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module kanari_framework::chain_id_test{
    use kanari_framework::chain_id;
    
    #[test]
    fun test_get_chain_id() {
        kanari_framework::genesis::init_for_test();
        let _id = chain_id::chain_id();
        
    }
}