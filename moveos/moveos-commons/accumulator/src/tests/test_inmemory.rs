// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::LeafCount;
use crate::inmemory::{InMemoryAccumulator, MerkleTreeInternalNode};
use crate::node_index::{FrozenSubtreeSiblingIterator, NodeIndex};
use crate::tree::AccumulatorTree;
use crate::tree_store::mock::MockAccumulatorStore;
use moveos_types::h256::{ACCUMULATOR_PLACEHOLDER_HASH, H256, sha3_256_of};
use proptest::{collection::vec, prelude::*};
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;

fn compute_parent_hash(left_hash: H256, right_hash: H256) -> H256 {
    if left_hash == *ACCUMULATOR_PLACEHOLDER_HASH && right_hash == *ACCUMULATOR_PLACEHOLDER_HASH {
        *ACCUMULATOR_PLACEHOLDER_HASH
    } else {
        MerkleTreeInternalNode::new(left_hash, right_hash).hash()
    }
}

/// Given a list of leaves, constructs the smallest accumulator that has all the leaves and
/// computes the hash of every node in the tree.
fn compute_hashes_for_all_positions(leaves: &[H256]) -> HashMap<NodeIndex, H256> {
    if leaves.is_empty() {
        return HashMap::new();
    }

    let mut current_leaves = leaves.to_vec();
    current_leaves.resize(
        leaves.len().next_power_of_two(),
        *ACCUMULATOR_PLACEHOLDER_HASH,
    );
    let mut position_to_hash = HashMap::new();
    let mut current_level = 0;

    while current_leaves.len() > 1 {
        assert!(current_leaves.len().is_power_of_two());

        let mut parent_leaves = vec![];
        for (index, _hash) in current_leaves.iter().enumerate().step_by(2) {
            let left_hash = current_leaves[index];
            let right_hash = current_leaves[index + 1];
            let parent_hash = compute_parent_hash(left_hash, right_hash);
            parent_leaves.push(parent_hash);

            let left_pos = NodeIndex::from_level_and_pos(current_level, index as u64);
            let right_pos = NodeIndex::from_level_and_pos(current_level, index as u64 + 1);
            assert_eq!(position_to_hash.insert(left_pos, left_hash), None);
            assert_eq!(position_to_hash.insert(right_pos, right_hash), None);
        }

        assert_eq!(current_leaves.len(), parent_leaves.len() << 1);
        current_leaves = parent_leaves;
        current_level += 1;
    }

    assert_eq!(
        position_to_hash.insert(
            NodeIndex::from_level_and_pos(current_level, 0),
            current_leaves[0],
        ),
        None,
    );
    position_to_hash
}

// Computes the root hash of an accumulator with given leaves.
fn compute_root_hash_naive(leaves: &[H256]) -> H256 {
    let position_to_hash = compute_hashes_for_all_positions(leaves);
    if position_to_hash.is_empty() {
        return *ACCUMULATOR_PLACEHOLDER_HASH;
    }

    let rightmost_leaf_index = leaves.len() as u64 - 1;
    *position_to_hash
        .get(&NodeIndex::root_from_leaf_index(rightmost_leaf_index))
        .expect("Root position should exist in the map.")
}

// Helper function to create a list of leaves.
fn create_leaves(nums: std::ops::Range<usize>) -> Vec<H256> {
    nums.map(|x| sha3_256_of(x.to_be_bytes().as_ref()))
        .collect()
}

#[test]
fn test_accumulator_append() {
    // expected_root_hashes[i] is the root hash of an accumulator that has the first i leaves.
    let expected_root_hashes = (0..100).map(|x| {
        let leaves = create_leaves(0..x);
        compute_root_hash_naive(&leaves)
    });

    let leaves = create_leaves(0..100);
    let mut accumulator = InMemoryAccumulator::default();
    // Append the leaves one at a time and check the root hashes match.
    for (i, (leaf, expected_root_hash)) in
        itertools::zip_eq(leaves.into_iter(), expected_root_hashes).enumerate()
    {
        assert_eq!(accumulator.root_hash(), expected_root_hash);
        assert_eq!(accumulator.num_leaves(), i as LeafCount);
        accumulator = accumulator.append(&[leaf]);
    }
}

#[test]
fn test_tree_and_inmemory_compare() {
    let mut rng = rand::thread_rng();
    let leaf_count = rng.gen_range(100..200);
    let leaves = create_leaves(0..leaf_count);
    let mut accumulator = InMemoryAccumulator::default();
    accumulator = accumulator.append(leaves.as_slice());
    let store = MockAccumulatorStore::new();
    let mut tree_accumulator = AccumulatorTree::new_empty(Arc::new(store));
    tree_accumulator.append(leaves.as_slice()).unwrap();
    tree_accumulator.flush().unwrap();

    assert_eq!(accumulator.root_hash(), tree_accumulator.root_hash);
    assert_eq!(
        accumulator.frozen_subtree_roots(),
        &tree_accumulator.get_frozen_subtree_roots()
    );
    assert_eq!(accumulator.num_leaves(), tree_accumulator.num_leaves);
}

#[test]
fn test_proof() {
    let mut rng = rand::thread_rng();
    let leaf_count = rng.gen_range(1000..2000);
    let leaves = create_leaves(0..leaf_count);
    let accumulator = InMemoryAccumulator::from_leaves(leaves.as_slice());
    let leaf_index = rng.gen_range(0..leaf_count as u64);
    let proof = InMemoryAccumulator::get_proof_from_leaves(leaves.as_slice(), leaf_index).unwrap();
    assert!(
        proof
            .verify(
                accumulator.root_hash(),
                leaves[leaf_index as usize],
                leaf_index
            )
            .is_ok(),
        "leaf_index {}, proof: {:?} verify failed",
        leaf_index,
        proof
    );
}

proptest! {
    #[test]
    fn test_accumulator_append_subtrees(
        hashes1 in vec(any::<u64>().prop_map(|h| { H256::from_low_u64_le(h) }), 0..100),
        hashes2 in vec(any::<u64>().prop_map(|h| { H256::from_low_u64_le(h) }), 0..100),
    ) {
        // Construct an accumulator with hashes1.
        let accumulator = InMemoryAccumulator::from_leaves(&hashes1);

        // Compute all the internal nodes in a bigger accumulator with combination of hashes1 and
        // hashes2.
        let mut all_hashes = hashes1.clone();
        all_hashes.extend_from_slice(&hashes2);
        let position_to_hash = compute_hashes_for_all_positions(&all_hashes);

        let subtree_hashes: Vec<_> =
            FrozenSubtreeSiblingIterator::new(hashes1.len() as LeafCount, all_hashes.len() as LeafCount)
                .filter_map(|pos| position_to_hash.get(&pos).cloned())
                .collect();
        let new_accumulator = accumulator
            .append_subtrees(&subtree_hashes, hashes2.len() as LeafCount)
            .unwrap();
        prop_assert_eq!(
            new_accumulator.root_hash(),
            compute_root_hash_naive(&all_hashes)
        );
    }
}
