// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use kanari_types::addresses::MOVEOS_STD_ADDRESS;
use kanari_types::crypto::KanariKeyPair;
use kanari_types::error::KanariError;
use kanari_types::framework::empty::Empty;
use kanari_types::framework::gas_coin::KARI;
use kanari_types::framework::transfer::TransferModule;
use kanari_types::test_utils::{random_string, random_string_with_size};
use kanari_types::transaction::KanariTransaction;
use kanari_types::transaction::kanari::KanariTransactionData;
use move_core_types::account_address::AccountAddress;
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::{ModuleId, TypeTag};
use move_core_types::u256::U256;
use move_package::BuildConfig;
use moveos_compiler::dependency_order::sort_by_dependency_order;
use moveos_types::move_std::string::MoveString;
use moveos_types::move_types::FunctionId;
use moveos_types::moveos_std::module_store::PackageData;
use moveos_types::state::MoveStructType;
use moveos_types::transaction::{FunctionCall, MoveAction};
use moveos_verifier::build::run_verifier;
use std::collections::BTreeMap;
use std::io::stderr;
use std::path::PathBuf;

#[derive(Debug)]
pub struct TestTransactionBuilder {
    pub kp: KanariKeyPair,
    pub sender: AccountAddress,
    pub sequence_number: u64,
}

impl TestTransactionBuilder {
    pub fn new(kp: KanariKeyPair) -> Self {
        let sender = kp.public().kanari_address().unwrap();
        Self {
            kp,
            sender: sender.into(),
            sequence_number: 0,
        }
    }

    pub fn sender(&self) -> AccountAddress {
        self.sender
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence_number
    }

    pub fn update_sequence_number(&mut self, sequence_number: u64) {
        self.sequence_number = sequence_number
    }

    pub fn new_function_call(
        &self,
        module: &'static str,
        function: &'static str,
        args: Vec<Vec<u8>>,
        ty_args: Vec<TypeTag>,
    ) -> MoveAction {
        let function_id = FunctionId::new(
            ModuleId::new(self.sender, Identifier::new(module).unwrap()),
            Identifier::new(function).unwrap(),
        );

        MoveAction::Function(FunctionCall {
            function_id,
            ty_args,
            args,
        })
    }

    pub fn call_empty_create(&self) -> MoveAction {
        MoveAction::new_function_call(Empty::empty_function_id(), vec![], vec![])
    }

    pub fn call_transfer_create(&self) -> MoveAction {
        TransferModule::create_transfer_coin_action(
            KARI::struct_tag(),
            AccountAddress::random(),
            U256::from(100u128),
        )
    }

    pub fn call_transfer_large_object_create(&self) -> MoveAction {
        let to = AccountAddress::random();
        self.new_function_call(
            "big_vector",
            "transfer",
            vec![bcs::to_bytes(&to).unwrap()],
            vec![],
        )
    }

    pub fn call_article_create(&self) -> MoveAction {
        let args = vec![
            bcs::to_bytes(&random_string_with_size(20)).expect("serialize title should success"),
            bcs::to_bytes(&random_string()).expect("serialize body should success"),
        ];

        self.new_function_call("simple_blog", "create_article", args, vec![])
    }

    pub fn call_article_create_with_size(&self, len: usize) -> MoveAction {
        let args = vec![
            bcs::to_bytes(&random_string_with_size(20)).expect("serialize title should success"),
            bcs::to_bytes(&random_string_with_size(len)).expect("serialize body should success"),
        ];

        self.new_function_call("simple_blog", "create_article", args, vec![])
    }

    pub fn new_publish_examples(
        &self,
        subpath: &'static str,
        named_address_key: Option<String>,
    ) -> Result<MoveAction> {
        let path = if let Ok(p) = std::env::var("KANARI_EXAMPLES_DIR") {
            let mut path = PathBuf::from(p);
            path.extend([subpath]);
            path
        } else {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.extend(["..", "..", "examples", subpath]);
            path
        };
        self.new_publish(path, named_address_key)
    }

    pub fn new_publish(
        &self,
        path: PathBuf,
        named_address_key: Option<String>,
    ) -> Result<MoveAction> {
        let pkg_data = self.build_package(path, named_address_key)?;

        let pkg_bytes = bcs::to_bytes(&pkg_data).unwrap();
        let args = bcs::to_bytes(&pkg_bytes).unwrap();
        let action = MoveAction::new_function_call(
            FunctionId::new(
                ModuleId::new(
                    MOVEOS_STD_ADDRESS,
                    Identifier::new("module_store".to_owned()).unwrap(),
                ),
                Identifier::new("publish_package_entry".to_owned()).unwrap(),
            ),
            vec![],
            vec![args],
        );
        Ok(action)
    }

    pub fn build_package(
        &self,
        package_path: PathBuf,
        named_address_key: Option<String>,
    ) -> Result<PackageData, anyhow::Error> {
        let mut config = BuildConfig::default();

        // Parse named addresses from context and update config
        if let Some(key) = named_address_key {
            let mut named_addresses = BTreeMap::<String, AccountAddress>::new();
            named_addresses.insert(key, self.sender);
            config.additional_named_addresses = named_addresses;
        };
        let config_cloned = config.clone();

        // Compile the package and run the verifier
        let mut package = config.compile_package_no_exit(&package_path, &mut stderr())?;
        run_verifier(package_path, config_cloned, &mut package)?;

        // Get the modules from the package
        let modules = package.root_modules_map();
        let empty_modules = modules.iter_modules_owned().is_empty();
        let pkg_address = if !empty_modules {
            let first_module = &modules.iter_modules_owned()[0];
            first_module.self_id().address().to_owned()
        } else {
            return Err(anyhow::Error::new(KanariError::MoveCompilationError(
                format!(
                    "compiling move modules error! Is the project or module empty: {:?}",
                    empty_modules,
                ),
            )));
        };

        // Initialize bundles vector and sort modules by dependency order
        let mut bundles: Vec<Vec<u8>> = vec![];
        let sorted_modules = sort_by_dependency_order(modules.iter_modules())?;
        // Serialize and collect module binaries into bundles
        for module in sorted_modules {
            let module_address = module.self_id().address().to_owned();
            if module_address != pkg_address {
                return Err(anyhow::Error::new(KanariError::MoveCompilationError(
                    format!(
                        "module's address ({:?}) not same as package module address {:?}",
                        module_address,
                        pkg_address.clone(),
                    ),
                )));
            };
            // verifier::verify_module(&module, &resolver)?;
            let mut binary: Vec<u8> = vec![];
            module.serialize(&mut binary)?;
            bundles.push(binary);
        }

        let pkg_data = PackageData::new(
            MoveString::from(package.compiled_package_info.package_name.as_str()),
            self.sender,
            bundles,
        );

        Ok(pkg_data)
    }

    pub fn build(&self, action: MoveAction) -> KanariTransactionData {
        KanariTransactionData::new_for_test(self.sender.into(), self.sequence_number, action)
    }

    pub fn build_and_sign(&self, action: MoveAction) -> Result<KanariTransaction> {
        let tx_data = self.build(action);
        Ok(tx_data.sign(&self.kp))
    }
}
