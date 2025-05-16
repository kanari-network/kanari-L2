// Copyright (c) KanariNetwork
// SPDX-License-Identifier: Apache-2.0

module kanari_framework::timestamp {
    use moveos_std::signer::module_signer;
    use moveos_std::timestamp::fast_forward_seconds_by_system;

    /// Just using to get module signer
    struct TimestampPlaceholder has key {}

    const ErrorUnsupportedChain:u64 = 1;

    /// Fast forwards the clock by the given number of seconds, but only if the chain is in local mode.
    public entry fun fast_forward_seconds_for_local(timestamp_seconds: u64) {
        assert!(kanari_framework::chain_id::is_local(), ErrorUnsupportedChain);
        let module_signer = module_signer<TimestampPlaceholder>();
        fast_forward_seconds_by_system(&module_signer, timestamp_seconds);
    }
}
