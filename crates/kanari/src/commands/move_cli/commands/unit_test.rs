// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use crate::commands::move_cli::serialized_success;
use async_trait::async_trait;
use clap::Parser;
use codespan_reporting::diagnostic::Severity;
use kanari_genesis::FrameworksGasParameters;
use kanari_types::error::{KanariError, KanariResult};
use kanari_types::genesis_config;
use move_cli::{Move, base::test};
use move_command_line_common::address::NumericalAddress;
use move_command_line_common::parser::NumberFormat;
use move_unit_test::extensions::set_extension_hook;
use move_vm_runtime::native_extensions::NativeContextExtensions;
use moveos_config::DataDirPath;
use moveos_object_runtime::runtime::{ObjectRuntime, ObjectRuntimeContext};
use moveos_stdlib::natives::moveos_stdlib::{
    event::NativeEventContext, move_module::NativeModuleContext,
};
use moveos_store::{MoveOSStore, load_feature_store_object};
use moveos_types::moveos_std::object::ObjectMeta;
use moveos_types::{moveos_std::tx_context::TxContext, state_resolver::RootObjectResolver};
use moveos_verifier::build::build_model_with_test_attr;
use moveos_verifier::metadata::run_extended_checks;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde_json::Value;
use std::rc::Rc;
use std::{collections::BTreeMap, path::PathBuf};
use termcolor::Buffer;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[group(skip)]
pub struct TestCommand {
    #[clap(flatten)]
    pub test: test::Test,

    /// Named addresses for the move binary
    ///
    /// Example: alice=0x1234, bob=0x5678
    ///
    /// Note: This will fail if there are duplicates in the Move.toml file remove those first.
    #[clap(long, value_parser = crate::utils::parse_map::< String, String >, default_value = "")]
    pub(crate) named_addresses: BTreeMap<String, String>,

    #[clap(flatten)]
    config_options: WalletContextOptions,

    #[clap(flatten)]
    move_args: Move,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<Option<Value>> for TestCommand {
    async fn execute(self) -> KanariResult<Option<Value>> {
        let context = self.config_options.build()?;

        let mut build_config = self.move_args.build_config;
        build_config
            .additional_named_addresses
            .extend(context.parse_and_resolve_addresses(self.named_addresses)?);

        let root_path = self
            .move_args
            .package_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        build_config.dev_mode = true;

        let resolution_graph = build_config
            .clone()
            .resolution_graph_for_package(&root_path, &mut Vec::new())?;

        let mut additional_named_address = BTreeMap::new();
        let _: Vec<_> = resolution_graph
            .extract_named_address_mapping()
            .map(|(name, addr)| {
                (additional_named_address.insert(
                    name.to_string(),
                    NumericalAddress::new(addr.into_bytes(), NumberFormat::Hex).into_inner(),
                ),)
            })
            .collect();

        let global_env = build_model_with_test_attr(&root_path, additional_named_address, None)?;

        let _ = run_extended_checks(&global_env);

        if global_env.diag_count(Severity::Warning) > 0 {
            let mut buffer = Buffer::ansi();
            global_env.report_diag(&mut buffer, Severity::Warning);
            let buffer_output = String::from_utf8_lossy(buffer.as_slice()).to_string();
            eprintln!("{}", buffer_output);
            if global_env.has_errors() {
                return Err(KanariError::from(anyhow::Error::msg(
                    "extended checks failed",
                )));
            }
        }

        //TODO define gas metering
        let cost_table = move_vm_test_utils::gas_schedule::INITIAL_COST_SCHEDULE.clone();
        let gas_parameter = FrameworksGasParameters::initial();
        let natives = gas_parameter.all_natives();
        set_extension_hook(Box::new(new_moveos_natives_runtime));
        self.test.execute(
            self.move_args.package_path,
            build_config,
            natives,
            Some(cost_table),
        )?;

        serialized_success(self.json)
    }
}

static MOVEOSSTORE: Lazy<(MoveOSStore, DataDirPath)> = Lazy::new(|| {
    let runtime = Runtime::new()
        .expect("Failed to create Tokio runtime when mock moveos store in move unit test");
    runtime.block_on(async { MoveOSStore::mock_moveos_store().unwrap() })
});

static RESOLVER: Lazy<Box<RootObjectResolver<MoveOSStore>>> = Lazy::new(|| {
    Box::new(RootObjectResolver::new(
        ObjectMeta::genesis_root(),
        &MOVEOSSTORE.0,
    ))
});

#[allow(clippy::arc_with_non_send_sync)]
fn new_moveos_natives_runtime(ext: &mut NativeContextExtensions) {
    let resolver = Lazy::force(&RESOLVER).as_ref();
    let object_runtime = Rc::new(RwLock::new(ObjectRuntime::genesis(
        TxContext::random_for_testing_only(),
        ObjectMeta::genesis_root(),
        resolver,
        genesis_config::G_LOCAL_CONFIG.genesis_objects.clone(),
    )));
    let feature_store = load_feature_store_object(resolver);
    let table_ext = ObjectRuntimeContext::new(object_runtime, feature_store);
    let module_ext = NativeModuleContext::new(resolver);
    let event_ext = NativeEventContext::default();
    ext.add(table_ext);
    ext.add(module_ext);
    ext.add(event_ext);
}
