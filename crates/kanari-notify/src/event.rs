// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use kanari_types::service_status::ServiceStatus;

#[derive(Default, Clone, Debug)]
pub struct GasUpgradeEvent {}

#[derive(Default, Clone, Debug)]
pub struct ServiceStatusEvent {
    pub status: ServiceStatus,
}
