// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

// // Copyright (c) The Diem Core Contributors
// // SPDX-License-Identifier: Apache-2.0
//
// use super::{
//     iterator::JellyfishMerkleIterator, mock_tree_store::MockTreeStore, test_helper::plus_one,
//     JellyfishMerkleTree,
// };
// use anyhow::Result;
// use diem_crypto::SMTNodeHash;
// use diem_types::{account_state_blob::AccountStateBlob, transaction::Version};
// use rand::{rngs::StdRng, SeedableRng};
// use std::{collections::BTreeMap, sync::Arc};
//
// #[test]
// fn test_iterator_same_version() {
//     for i in (1..100).step_by(11) {
//         test_n_leaves_same_version(i);
//     }
// }
//
// #[test]
// fn test_iterator_multiple_versions() {
//     test_n_leaves_multiple_versions(50);
// }
//
// fn test_n_leaves_same_version(n: usize) {
//     let db = Arc::new(MockTreeStore::default());
//     let tree = JellyfishMerkleTree::new(&*db);
//
//     let mut rng = StdRng::from_seed([1; 32]);
//
//     let mut btree = BTreeMap::new();
//     for i in 0..n {
//         let key = SMTNodeHash::random_with_rng(&mut rng);
//         let value = AccountStateBlob::from(i.to_be_bytes().to_vec());
//         assert_eq!(btree.insert(key, value), None);
//     }
//
//     let (_root_hash, batch) = tree
//         .put_blob_set(btree.clone().into_iter().collect(), 0 /* version */)
//         .unwrap();
//     db.write_tree_update_batch(batch).unwrap();
//
//     run_tests(db, &btree, 0 /* version */);
// }
//
// fn test_n_leaves_multiple_versions(n: usize) {
//     let db = Arc::new(MockTreeStore::default());
//     let tree = JellyfishMerkleTree::new(&*db);
//
//     let mut rng = StdRng::from_seed([1; 32]);
//
//     let mut btree = BTreeMap::new();
//     for i in 0..n {
//         let key = SMTNodeHash::random_with_rng(&mut rng);
//         let value = AccountStateBlob::from(i.to_be_bytes().to_vec());
//         assert_eq!(btree.insert(key, value.clone()), None);
//         let (_root_hash, batch) = tree.put_blob_set(vec![(key, value)], i as Version).unwrap();
//         db.write_tree_update_batch(batch).unwrap();
//         run_tests(Arc::clone(&db), &btree, i as Version);
//     }
// }
//
// fn run_tests(
//     db: Arc<MockTreeStore>,
//     btree: &BTreeMap<SMTNodeHash, AccountStateBlob>,
//     version: Version,
// ) {
//     {
//         let iter =
//             JellyfishMerkleIterator::new(Arc::clone(&db), version, SMTNodeHash::zero()).unwrap();
//         assert_eq!(
//             iter.collect::<Result<Vec<_>>>().unwrap(),
//             btree.clone().into_iter().collect::<Vec<_>>(),
//         );
//     }
//
//     for i in 0..btree.len() {
//         let ith_key = *btree.keys().nth(i).unwrap();
//
//         {
//             let iter = JellyfishMerkleIterator::new(Arc::clone(&db), version, ith_key).unwrap();
//             assert_eq!(
//                 iter.collect::<Result<Vec<_>>>().unwrap(),
//                 btree.clone().into_iter().skip(i).collect::<Vec<_>>(),
//             );
//         }
//
//         {
//             let ith_key_plus_one = plus_one(ith_key);
//             let iter =
//                 JellyfishMerkleIterator::new(Arc::clone(&db), version, ith_key_plus_one).unwrap();
//             assert_eq!(
//                 iter.collect::<Result<Vec<_>>>().unwrap(),
//                 btree.clone().into_iter().skip(i + 1).collect::<Vec<_>>(),
//             );
//         }
//     }
//
//     {
//         let iter = JellyfishMerkleIterator::new(
//             Arc::clone(&db),
//             version,
//             SMTNodeHash::new([0xFF; SMTNodeHash::LEN]),
//         )
//         .unwrap();
//         assert_eq!(iter.collect::<Result<Vec<_>>>().unwrap(), vec![]);
//     }
// }
