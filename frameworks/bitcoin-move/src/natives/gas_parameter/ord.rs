// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::natives::ord::GasParameters;
use kanari_framework::natives::gas_parameter::native::MUL;

kanari_framework::natives::gas_parameter::native::define_gas_parameters_for_natives!(GasParameters, "ord", [
    [.from_witness.base, "from_witness.base", 10000 * MUL],
    [.from_witness.per_byte, "from_witness.per_byte", 50 * MUL],
]);
