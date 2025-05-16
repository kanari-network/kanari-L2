// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

mod adapter;
mod avail;
mod celestia;
mod manager;
mod opendal;

pub use self::adapter::AdapterSubmitStat;

pub use self::manager::OpenDABackendManager;
use kanari_config::da_config::OpenDAScheme;

pub fn derive_identifier(scheme: OpenDAScheme) -> String {
    format!("openda-{}", scheme)
}
