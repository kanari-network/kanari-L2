// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

module kanari_framework::address_mapping{
    
    use std::option::{Self, Option};
    use std::vector;
    use moveos_std::core_addresses;
    use moveos_std::object::{Self, Object};
    use kanari_framework::multichain_address::{Self, MultiChainAddress};
    use kanari_framework::bitcoin_address::{Self, BitcoinAddress};

    friend kanari_framework::genesis;
    friend kanari_framework::bitcoin_validator;
    friend kanari_framework::transaction_validator;
    friend kanari_framework::transfer;
    
    const ErrorMultiChainAddressInvalid: u64 = 1;
    const ErrorUnsupportedAddress: u64 = 2;

    const NAMED_MAPPING_INDEX: u64 = 0;
    const NAMED_REVERSE_MAPPING_INDEX: u64 = 1;

    /// Mapping from multi-chain address to kanari address
    /// Not including Bitcoin address, because Bitcoin address can directly hash to kanari address
    /// The mapping record is the object field, key is the multi-chain address, value is the kanari address
    struct MultiChainAddressMapping has key{
        _placeholder: bool,
    }
    
    /// Mapping from kanari address to bitcoin address, other chain can use new table
    /// The mapping record is the object field, key is the kanari address, value is the Bitcoin address
    struct KanariToBitcoinAddressMapping has key{
        _placeholder: bool,
    }

    public(friend) fun genesis_init(_genesis_account: &signer) {
        let multichain_mapping_id = object::named_object_id<MultiChainAddressMapping>();
        if(!object::exists_object(multichain_mapping_id)){
            let multichain_mapping = object::new_named_object(MultiChainAddressMapping{
                _placeholder: false
            });
            object::transfer_extend(multichain_mapping, @kanari_framework);
        };
        let kanari_to_bitcoin_mapping_id = object::named_object_id<KanariToBitcoinAddressMapping>();
        if(!object::exists_object(kanari_to_bitcoin_mapping_id)){
            let kanari_to_bitcoin_mapping = object::new_named_object(KanariToBitcoinAddressMapping{
                _placeholder: false
            });
            object::transfer_extend(kanari_to_bitcoin_mapping, @kanari_framework);
        };
    }

    fun borrow_multichain() : &Object<MultiChainAddressMapping> {
        let object_id = object::named_object_id<MultiChainAddressMapping>();
        object::borrow_object<MultiChainAddressMapping>(object_id)
    }

    fun borrow_multichain_mut() : &mut Object<MultiChainAddressMapping> {
        let object_id = object::named_object_id<MultiChainAddressMapping>();
        object::borrow_mut_object_extend<MultiChainAddressMapping>(object_id)
    }

    fun borrow_kanari_to_bitcoin() : &Object<KanariToBitcoinAddressMapping> {
        let object_id = object::named_object_id<KanariToBitcoinAddressMapping>();
        object::borrow_object<KanariToBitcoinAddressMapping>(object_id)
    }

    fun borrow_kanari_to_bitcoin_mut() : &mut Object<KanariToBitcoinAddressMapping> {
        let object_id = object::named_object_id<KanariToBitcoinAddressMapping>();
        object::borrow_mut_object_extend<KanariToBitcoinAddressMapping>(object_id)
    }

    fun resolve_address(obj: &Object<MultiChainAddressMapping>, maddress: MultiChainAddress): Option<address> {
        if (multichain_address::is_kanari_address(&maddress)) {
            return option::some(multichain_address::into_kanari_address(maddress))
        };
        if (multichain_address::is_bitcoin_address(&maddress)) {
            return option::some(bitcoin_address::to_kanari_address(&multichain_address::into_bitcoin_address(maddress)))
        };

        if(object::contains_field(obj, maddress)){
            let addr = object::borrow_field(obj, maddress);
            option::some(*addr)
        }else{
            option::none()
        }
    }

    fun resolve_bitcoin_address(obj: &Object<KanariToBitcoinAddressMapping>, kanari_address: address): Option<BitcoinAddress> {
        if(object::contains_field(obj, kanari_address)){
            let addr = object::borrow_field(obj, kanari_address);
            option::some(*addr)
        }else{
            option::none()
        }
    }

    fun exists_mapping_address(obj: &Object<MultiChainAddressMapping>, maddress: MultiChainAddress): bool {
        if (multichain_address::is_kanari_address(&maddress) || multichain_address::is_bitcoin_address(&maddress)) {
            return true
        };
        object::contains_field(obj, maddress)
    }

    /// Resolve a multi-chain address to a kanari address
    public fun resolve(maddress: MultiChainAddress): Option<address> {
        let am = Self::borrow_multichain();
        Self::resolve_address(am, maddress)
    }

    /// Resolve a kanari address to a bitcoin address
    public fun resolve_bitcoin(kanari_address: address): Option<BitcoinAddress> {
        let am = Self::borrow_kanari_to_bitcoin();
        Self::resolve_bitcoin_address(am, kanari_address)
    }

    /// Resolve a batch kanari addresses to bitcoin addresses
    public fun resolve_bitcoin_batch(kanari_addresses: vector<address>): vector<BitcoinAddress> {
        let am = Self::borrow_kanari_to_bitcoin();
        vector::map(kanari_addresses, |kanari_address| {
            let addr_opt = Self::resolve_bitcoin_address(am, kanari_address);
            if(option::is_none(&addr_opt)){
                bitcoin_address::empty()
            }else{
                option::destroy_some(addr_opt)
            }
        })
    } 

    /// Check if a multi-chain address is bound to a kanari address
    public fun exists_mapping(maddress: MultiChainAddress): bool {
        let obj = Self::borrow_multichain();
        Self::exists_mapping_address(obj, maddress)
    }

    public fun bind_bitcoin_address_internal(kanari_address: address, btc_address: BitcoinAddress) {
        // bitcoin address to kanari address do not need to record, we just record kanari address to bitcoin address
        let obj = Self::borrow_kanari_to_bitcoin_mut();
        if(!object::contains_field(obj, kanari_address)){
            object::add_field(obj, kanari_address, btc_address);
        }
    }

    public fun bind_bitcoin_address_by_system(system: &signer, kanari_address: address, btc_address: BitcoinAddress) {
        core_addresses::assert_system_reserved(system);
        Self::bind_bitcoin_address_internal(kanari_address, btc_address);
    }


    /// Bind a bitcoin address to a kanari address
    /// We can calculate the kanari address from bitcoin address
    /// So we call this function for record kanari address to bitcoin address mapping
    public fun bind_bitcoin_address(btc_address: BitcoinAddress){
        let kanari_addr = bitcoin_address::to_kanari_address(&btc_address);
        Self::bind_bitcoin_address_internal(kanari_addr, btc_address);
    }

    #[test_only]
    use std::string;

    #[test]
    fun test_address_mapping_for_bitcoin(){
        let genesis_account = moveos_std::signer::module_signer<KanariToBitcoinAddressMapping>();
        genesis_init(&genesis_account);
        let btc_addr = bitcoin_address::from_string(&string::utf8(b"bc1p8xpjpkc9uzj2dexcxjg9sw8lxje85xa4070zpcys589e3rf6k20qm6gjrt"));
        bind_bitcoin_address(btc_addr);
        let kanari_addr = bitcoin_address::to_kanari_address(&btc_addr);
        let resolved_addr = resolve_bitcoin(kanari_addr);
        assert!(resolved_addr == option::some(btc_addr), 1);
    }

    #[test]
    fun test_address_mapping_for_bitcoin_batch(){
        let genesis_account = moveos_std::signer::module_signer<KanariToBitcoinAddressMapping>();
        genesis_init(&genesis_account);
        let btc_addr = bitcoin_address::from_string(&string::utf8(b"bc1p8xpjpkc9uzj2dexcxjg9sw8lxje85xa4070zpcys589e3rf6k20qm6gjrt"));
        bind_bitcoin_address(btc_addr);
        let kanari_addr = bitcoin_address::to_kanari_address(&btc_addr);
        let addresses = vector[kanari_addr, @0x42];
        let resolved_addrs = resolve_bitcoin_batch(addresses);
        assert!(vector::length(&resolved_addrs) == 2, 1);
        assert!(*vector::borrow(&resolved_addrs, 0) == btc_addr, 1);
        assert!(bitcoin_address::is_empty(vector::borrow(&resolved_addrs, 1)), 1);
    }


}
