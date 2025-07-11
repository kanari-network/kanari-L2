// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::Stdlib;
use crate::release_dir;
use crate::stdlib_configs::build_stdlib;
use crate::stdlib_version::StdlibVersion;
use anyhow::{Result, bail, ensure};
use framework_types::addresses::KANARI_NURSERY_ADDRESS;
use itertools::Itertools;
use move_binary_format::{CompiledModule, compatibility::Compatibility, errors::PartialVMResult};
use moveos_types::moveos_std::module_store::PackageData;
use std::collections::HashMap;
use tracing::{debug, info, warn};

pub fn release_latest() -> Result<Vec<String>> {
    release(StdlibVersion::Latest, false)
}

pub fn release(version: StdlibVersion, check_compatibility: bool) -> Result<Vec<String>> {
    let mut warnings = vec![];
    let pre_version = match version {
        StdlibVersion::Version(version_num) => {
            if version_num > 1 {
                Some(StdlibVersion::Version(version_num - 1))
            } else {
                None
            }
        }
        StdlibVersion::Latest => {
            // The latest version must be compatible with the max released version
            let max_version = current_max_version();
            if max_version > 0 {
                Some(StdlibVersion::Version(max_version))
            } else {
                None
            }
        }
    };

    if let Some(pre_version) = pre_version {
        if pre_version == version {
            bail!(
                "Version {:?} is already released. Please remove the dir and release again.",
                version.as_string()
            );
        }
    }
    version.create_dir()?;
    let curr_stdlib = build_stdlib(!version.is_latest())?;
    // check compatibility
    if let Some(pre_version) = pre_version {
        info!(
            "Checking compatibility between version {:?} and version {:?}",
            version.as_string(),
            pre_version.as_string()
        );
        let prev_stdlib = pre_version.load_from_file()?;
        match check_stdlib_compatibility(&curr_stdlib, &prev_stdlib) {
            Ok(_) => {}
            Err(err) => {
                // if check_compatibility is true, we should bail out and stop the release
                // otherwise, we just log the error and continue the release
                if check_compatibility {
                    bail!(
                        "Version {:?} is incompatible with previous version {:?}: {:?}",
                        version.as_string(),
                        pre_version.as_string(),
                        err
                    );
                } else {
                    let msg = format!(
                        "Version {:?} is incompatible with previous version {:?}: {:?}",
                        version.as_string(),
                        pre_version.as_string(),
                        err
                    );
                    warnings.push(msg);
                }
            }
        }
    }

    version.save(&curr_stdlib)?; // save the whole stdlib(legacy format).
    version.save_each_package(&curr_stdlib)?;
    info!(
        "Release stdlib version {:?} successfully.",
        version.as_string()
    );
    Ok(warnings)
}

/// Read max version number except `latest` from stdlib release dir
fn current_max_version() -> u64 {
    let mut max_version = 0;
    if !release_dir().exists() {
        return max_version;
    }
    for entry in release_dir().read_dir().unwrap() {
        let entry = entry.unwrap();
        let dirname = entry.file_name();
        if let Some(dirname_str) = dirname.to_str() {
            if let Ok(version) = dirname_str.parse::<u64>() {
                if version > max_version {
                    max_version = version;
                }
            }
        }
    }
    max_version
}

/// Check whether the new stdlib is compatible with the old stdlib
fn check_stdlib_compatibility(curr_stdlib: &Stdlib, prev_stdlib: &Stdlib) -> Result<()> {
    check_modules_compat(
        curr_stdlib.all_modules()?,
        prev_stdlib.all_modules()?,
        false,
    )
}

pub fn check_modules_compat(
    new_modules: Vec<CompiledModule>,
    old_modules: Vec<CompiledModule>,
    allow_deleted_module: bool,
) -> Result<()> {
    let new_modules_map = new_modules
        .into_iter()
        .map(|module| (module.self_id(), module))
        .collect::<HashMap<_, _>>();
    let old_modules_map = old_modules
        .into_iter()
        .map(|module| (module.self_id(), module))
        .collect::<HashMap<_, _>>();

    let incompatible_module_ids = new_modules_map
        .values()
        .filter_map(|module| {
            let module_id = module.self_id();
            if module_id.address() == &KANARI_NURSERY_ADDRESS {
                // ignore nursery module
                return None;
            }
            if let Some(old_module) = old_modules_map.get(&module_id) {
                let compatibility = check_compiled_module_compat(old_module, module);
                if let Err(err) = compatibility {
                    warn!(
                        "Module {:?} is incompatible with previous version: {:?}",
                        module_id, err
                    );
                    Some(module_id)
                } else {
                    debug!(
                        "Module {:?} is compatible with previous version.",
                        module_id
                    );
                    None
                }
            } else {
                info!("Module {:?} is new module.", module_id);
                None
            }
        })
        .collect::<Vec<_>>();

    ensure!(
        incompatible_module_ids.is_empty(),
        "Modules {} is incompatible with previous version!",
        incompatible_module_ids
            .into_iter()
            .map(|module_id| module_id.to_string())
            .join(","),
    );

    let deleted_module_ids = old_modules_map
        .keys()
        .filter_map(|module_id| {
            if !new_modules_map.contains_key(module_id) {
                Some(module_id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if !allow_deleted_module {
        ensure!(
            deleted_module_ids.is_empty(),
            "Modules {} is deleted!",
            deleted_module_ids
                .into_iter()
                .map(|module_id| module_id.to_string())
                .join(",")
        );
    } else if !deleted_module_ids.is_empty() {
        warn!(
            "Modules {} is deleted in local, but them still on the Chain!. If you want to deprecated them, please abort the all public functions in the module.",
            deleted_module_ids
                .into_iter()
                .map(|module_id| module_id.to_string())
                .join(",")
        );
    }
    Ok(())
}

pub fn check_package_compat(
    new_package_data: PackageData,
    pre_package_data: PackageData,
) -> Result<()> {
    let new_modules = new_package_data.compiled_modules()?;
    let pre_modules = pre_package_data.compiled_modules()?;
    check_modules_compat(new_modules, pre_modules, false)
}

/// check module compatibility
fn check_compiled_module_compat(
    old_module: &CompiledModule,
    new_module: &CompiledModule,
) -> PartialVMResult<()> {
    if new_module == old_module {
        return Ok(());
    }
    debug!(
        "Checking compatibility between module {:?} and module {:?}",
        new_module.self_id(),
        old_module.self_id()
    );
    //We enable all compatibility checks, do not allow friend functions break after the issue
    //https://github.com/rooch-network/rooch/pull/3465
    let compat = Compatibility::new(true, true, true, true);
    compat.check(old_module, new_module)
}
