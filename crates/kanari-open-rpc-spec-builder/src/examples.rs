// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use kanari_open_rpc::ExamplePairing;
use rand::rngs::StdRng;

// TODO: examples

#[allow(dead_code)]
struct Examples {
    function_name: String,
    examples: Vec<ExamplePairing>,
}

#[derive(serde::Serialize)]
#[allow(dead_code)]
struct Value {
    value: String,
}

#[allow(dead_code)]
impl Examples {
    fn new(name: &str, examples: Vec<ExamplePairing>) -> Self {
        Self {
            function_name: name.to_string(),
            examples,
        }
    }
}

#[allow(dead_code)]
pub struct RpcExampleProvider {
    rng: StdRng,
}

// impl RpcExampleProvider {
//     pub fn new() -> Self {
//         Self {
//             rng: StdRng::from_seed([0; 32]),
//         }
//     }

//     pub fn examples(&mut self) -> BTreeMap<String, Vec<ExamplePairing>> {
//         vec![]
//         // [
//         // ]
//         // .into_iter()
//         // .map(|example| (example.function_name, example.examples))
//         // .collect()
//     }
// }
