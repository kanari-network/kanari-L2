// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use accumulator::accumulator_info::AccumulatorInfo;
use accumulator::{Accumulator, MerkleAccumulator};
use anyhow::{Result, ensure};
use framework_builder::Stdlib;
use framework_builder::stdlib_version::StdlibVersion;
use include_dir::{Dir, include_dir};
use kanari_db::KanariDB;
use kanari_framework::KANARI_FRAMEWORK_ADDRESS;
use kanari_framework::natives::gas_parameter::gas_member::{
    FromOnChainGasSchedule, InitialGasSchedule, ToOnChainGasSchedule,
};
use kanari_indexer::store::traits::IndexerStoreTrait;
use kanari_store::state_store::StateStore;
use kanari_types::bitcoin::genesis::BitcoinGenesisContext;
use kanari_types::error::GenesisError;
use kanari_types::framework::chain_id::ChainID;
use kanari_types::indexer::event::IndexerEvent;
use kanari_types::indexer::state::{
    IndexerObjectStateChangeSet, IndexerObjectStatesIndexGenerator, handle_object_change,
};
use kanari_types::indexer::transaction::IndexerTransaction;
use kanari_types::into_address::IntoAddress;
use kanari_types::kanari_network::{BuiltinChainID, KanariNetwork};
use kanari_types::sequencer::SequencerInfo;
use kanari_types::transaction::kanari::KanariTransaction;
use kanari_types::transaction::{LedgerTransaction, LedgerTxData};
use move_core_types::gas_algebra::{InternalGas, InternalGasPerArg};
use move_core_types::value::MoveTypeLayout;
use move_core_types::{account_address::AccountAddress, identifier::Identifier};
use move_vm_runtime::native_functions::NativeFunction;
use moveos::gas::table::VMGasParameters;
use moveos::moveos::{MoveOS, MoveOSConfig};
use moveos_stdlib::natives::moveos_stdlib::base64::EncodeDecodeGasParametersOption;
use moveos_stdlib::natives::moveos_stdlib::event::EmitWithHandleGasParameters;
use moveos_stdlib::natives::moveos_stdlib::object::ListFieldsGasParametersOption;
use moveos_store::MoveOSStore;
use moveos_types::genesis_info::GenesisInfo;
use moveos_types::h256::H256;
use moveos_types::move_std::string::MoveString;
use moveos_types::moveos_std::gas_schedule::{GasEntry, GasSchedule, GasScheduleConfig};
use moveos_types::moveos_std::object::ObjectMeta;
use moveos_types::state::{ObjectState, StateChangeSetExt};
use moveos_types::transaction::{
    GenesisRawTransactionOutput, MoveAction, MoveOSTransaction, RawTransactionOutput,
};
use moveos_types::{h256, state_resolver};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use moveos_stdlib::natives::moveos_stdlib::ability::GetAbilitiesGasParameters;
use std::path::PathBuf;
use std::str::FromStr;
use std::{fs::File, io::Write, path::Path};

pub static KANARI_LOCAL_GENESIS: Lazy<KanariGenesisV2> = Lazy::new(|| {
    let network: KanariNetwork = BuiltinChainID::Local.into();
    KanariGenesisV2::build(network).expect("build kanari genesis failed")
});
pub const LATEST_GAS_SCHEDULE_VERSION: u64 = GAS_SCHEDULE_RELEASE_V3;
// update the gas config for function calling
pub const GAS_SCHEDULE_RELEASE_V1: u64 = 1;

pub const GAS_SCHEDULE_RELEASE_V2: u64 = 2;
pub const GAS_SCHEDULE_RELEASE_V3: u64 = 3;

pub(crate) const STATIC_GENESIS_DIR: Dir = include_dir!("released");

pub fn load_genesis_from_binary(chain_id: BuiltinChainID) -> Result<Option<KanariGenesisV2>> {
    STATIC_GENESIS_DIR
        .get_file(chain_id.chain_name())
        .map(|f| {
            let genesis = KanariGenesisV2::decode(f.contents())?;
            Ok(genesis)
        })
        .transpose()
}

pub fn release_dir() -> PathBuf {
    path_in_crate("released")
}

pub fn genesis_file(chain_id: BuiltinChainID) -> PathBuf {
    release_dir().join(chain_id.chain_name())
}

pub struct FrameworksGasParameters {
    pub max_gas_amount: u64,
    pub vm_gas_params: VMGasParameters,
    pub kanari_framework_gas_params: kanari_framework::natives::NativeGasParameters,
    pub bitcoin_move_gas_params: bitcoin_move::natives::GasParameters,
    pub kanari_nursery_gas_params: Option<kanari_nursery::natives::GasParameters>,
}

impl FrameworksGasParameters {
    pub fn initial() -> Self {
        Self {
            max_gas_amount: GasScheduleConfig::INITIAL_MAX_GAS_AMOUNT,
            vm_gas_params: VMGasParameters::initial(),
            kanari_framework_gas_params: kanari_framework::natives::NativeGasParameters::initial(),
            bitcoin_move_gas_params: bitcoin_move::natives::GasParameters::initial(),
            kanari_nursery_gas_params: Some(kanari_nursery::natives::GasParameters::initial()),
        }
    }

    pub fn v1() -> Self {
        let mut gas_parameter = Self {
            max_gas_amount: GasScheduleConfig::INITIAL_MAX_GAS_AMOUNT,
            vm_gas_params: VMGasParameters::initial(),
            kanari_framework_gas_params: kanari_framework::natives::NativeGasParameters::initial(),
            bitcoin_move_gas_params: bitcoin_move::natives::GasParameters::initial(),
            kanari_nursery_gas_params: Some(kanari_nursery::natives::GasParameters::initial()),
        };

        gas_parameter
            .kanari_framework_gas_params
            .moveos_stdlib
            .base64
            .encode = EncodeDecodeGasParametersOption {
            base: Some(1000.into()),
            per_byte: Some(30.into()),
        };

        gas_parameter
            .kanari_framework_gas_params
            .moveos_stdlib
            .base64
            .decode = EncodeDecodeGasParametersOption {
            base: Some(1000.into()),
            per_byte: Some(30.into()),
        };

        gas_parameter
    }

    pub fn v2() -> Self {
        let mut v1_gas_parameter = FrameworksGasParameters::v1();

        v1_gas_parameter
            .vm_gas_params
            .instruction_gas_parameter
            .call_base = InternalGas::new(167);
        v1_gas_parameter
            .vm_gas_params
            .instruction_gas_parameter
            .call_per_arg = InternalGasPerArg::new(15);
        v1_gas_parameter
            .vm_gas_params
            .instruction_gas_parameter
            .call_per_local = InternalGasPerArg::new(15);
        v1_gas_parameter
            .vm_gas_params
            .instruction_gas_parameter
            .call_generic_base = InternalGas::new(167);
        v1_gas_parameter
            .vm_gas_params
            .instruction_gas_parameter
            .call_generic_per_arg = InternalGasPerArg::new(15);
        v1_gas_parameter
            .vm_gas_params
            .instruction_gas_parameter
            .call_generic_per_local = InternalGasPerArg::new(15);
        v1_gas_parameter
            .vm_gas_params
            .instruction_gas_parameter
            .call_generic_per_ty_arg = InternalGasPerArg::new(15);

        v1_gas_parameter
    }

    pub fn v3() -> Self {
        let mut v2_gas_parameter = FrameworksGasParameters::v2();

        v2_gas_parameter
            .kanari_framework_gas_params
            .moveos_stdlib
            .object_list_field_keys
            .list_field_keys = ListFieldsGasParametersOption::init(1000.into(), 150.into());

        v2_gas_parameter
    }

    pub fn v4() -> Self {
        let mut v3_gas_parameter = FrameworksGasParameters::v3();

        v3_gas_parameter
            .kanari_framework_gas_params
            .moveos_stdlib
            .events
            .emit_with_handle = EmitWithHandleGasParameters::init(1000.into(), 150.into());

        v3_gas_parameter
    }

    pub fn v5() -> Self {
        let mut v4_gas_parameter = FrameworksGasParameters::v4();

        v4_gas_parameter
            .kanari_framework_gas_params
            .moveos_stdlib
            .ability
            .get_abilities = GetAbilitiesGasParameters::init(1000.into(), 150.into());

        v4_gas_parameter
    }

    pub fn v6() -> Self {
        let mut v5_gas_parameter = FrameworksGasParameters::v5();

        v5_gas_parameter
            .kanari_framework_gas_params
            .ecdsa_r1
            .verify = kanari_framework::natives::kanari_framework::crypto::ecdsa_r1::FromBytesGasParameters::init(1000.into(), 30.into());

        v5_gas_parameter.kanari_framework_gas_params.rs256.verify =
            kanari_framework::natives::kanari_framework::crypto::rs256::VerifyGasParameters::init(
                1000.into(),
                30.into(),
            );

        v5_gas_parameter
            .kanari_framework_gas_params
            .rs256
            .verify_prehash =
            kanari_framework::natives::kanari_framework::crypto::rs256::VerifyGasParameters::init(
                1000.into(),
                30.into(),
            );

        v5_gas_parameter
    }

    pub fn latest() -> Self {
        FrameworksGasParameters::v6()
    }

    pub fn to_gas_schedule_config(&self, chain_id: ChainID) -> GasScheduleConfig {
        let mut entries = self.vm_gas_params.to_on_chain_gas_schedule();
        entries.extend(self.kanari_framework_gas_params.to_on_chain_gas_schedule());
        entries.extend(self.bitcoin_move_gas_params.to_on_chain_gas_schedule());

        if chain_id == BuiltinChainID::Dev.chain_id()
            || chain_id == BuiltinChainID::Local.chain_id()
        {
            if let Some(gas_params) = self.kanari_nursery_gas_params.clone() {
                entries.extend(gas_params.to_on_chain_gas_schedule());
            }
        }

        GasScheduleConfig {
            max_gas_amount: self.max_gas_amount,
            entries: entries
                .into_iter()
                .map(|(key, val)| GasEntry {
                    key: MoveString::from_str(key.as_str()).expect("GasEntry key must be ascii"),
                    val,
                })
                .collect(),
        }
    }

    pub fn load_from_chain(state_resolver: &dyn state_resolver::StateResolver) -> Result<Self> {
        let gas_schedule_state = state_resolver
            .get_object(&GasSchedule::gas_schedule_object_id())?
            .ok_or_else(|| anyhow::anyhow!("Gas schedule object not found"))?;
        let gas_schedule = gas_schedule_state.into_object::<GasSchedule>()?;
        Self::load_from_gas_entries(
            gas_schedule.value.max_gas_amount,
            gas_schedule.value.entries,
        )
    }

    pub fn load_from_gas_config(gas_config: &GasScheduleConfig) -> Result<Self> {
        Self::load_from_gas_entries(gas_config.max_gas_amount, gas_config.entries.clone())
    }

    pub fn load_from_gas_entries(max_gas_amount: u64, entries: Vec<GasEntry>) -> Result<Self> {
        let entries = entries
            .into_iter()
            .map(|entry| (entry.key.to_string(), entry.val))
            .collect::<BTreeMap<_, _>>();
        let vm_gas_parameter = VMGasParameters::from_on_chain_gas_schedule(&entries)
            .ok_or_else(|| anyhow::anyhow!("Failed to load vm gas parameters"))?;
        let kanari_framework_gas_params =
            kanari_framework::natives::NativeGasParameters::from_on_chain_gas_schedule(&entries)
                .ok_or_else(|| anyhow::anyhow!("Failed to load kanari framework gas parameters"))?;
        let bitcoin_move_gas_params =
            bitcoin_move::natives::GasParameters::from_on_chain_gas_schedule(&entries)
                .ok_or_else(|| anyhow::anyhow!("Failed to load bitcoin move gas parameters"))?;
        let kanari_nursery_gas_params =
            kanari_nursery::natives::GasParameters::from_on_chain_gas_schedule(&entries);
        Ok(Self {
            max_gas_amount,
            vm_gas_params: vm_gas_parameter,
            kanari_framework_gas_params,
            bitcoin_move_gas_params,
            kanari_nursery_gas_params,
        })
    }

    pub fn all_natives(&self) -> Vec<(AccountAddress, Identifier, Identifier, NativeFunction)> {
        let mut kanari_framework_native_tables =
            kanari_framework::natives::all_natives(self.kanari_framework_gas_params.clone());
        let bitcoin_move_native_table =
            bitcoin_move::natives::all_natives(self.bitcoin_move_gas_params.clone());

        if let Some(gas_params) = self.kanari_nursery_gas_params.clone() {
            let kanari_nursery_native_table = kanari_nursery::natives::all_natives(gas_params);
            kanari_framework_native_tables.extend(kanari_nursery_native_table);
        }

        kanari_framework_native_tables.extend(bitcoin_move_native_table);
        kanari_framework_native_tables
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KanariGenesis {
    /// The genesis tx output
    pub tx_output: GenesisRawTransactionOutput,
    pub initial_gas_config: GasScheduleConfig,
    pub genesis_objects: Vec<(ObjectState, MoveTypeLayout)>,
    pub genesis_tx: KanariTransaction,
    pub genesis_moveos_tx: MoveOSTransaction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KanariGenesisV2 {
    /// The genesis tx output
    pub tx_output: RawTransactionOutput,
    pub initial_gas_config: GasScheduleConfig,
    pub genesis_objects: Vec<(ObjectState, MoveTypeLayout)>,
    pub genesis_tx: KanariTransaction,
    pub genesis_moveos_tx: MoveOSTransaction,
}

impl From<KanariGenesis> for KanariGenesisV2 {
    fn from(genesis: KanariGenesis) -> Self {
        {
            KanariGenesisV2 {
                tx_output: genesis.tx_output.into(),
                initial_gas_config: genesis.initial_gas_config,
                genesis_objects: genesis.genesis_objects,
                genesis_tx: genesis.genesis_tx,
                genesis_moveos_tx: genesis.genesis_moveos_tx,
            }
        }
    }
}

impl From<KanariGenesisV2> for KanariGenesis {
    fn from(genesis: KanariGenesisV2) -> Self {
        {
            KanariGenesis {
                tx_output: genesis.tx_output.into(),
                initial_gas_config: genesis.initial_gas_config,
                genesis_objects: genesis.genesis_objects,
                genesis_tx: genesis.genesis_tx,
                genesis_moveos_tx: genesis.genesis_moveos_tx,
            }
        }
    }
}

impl KanariGenesis {
    // released genesis file (testnet and mainnet) must by decode by KanariGenesis
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        bcs::from_bytes(bytes).map_err(Into::into)
    }

    pub fn encode(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("KanariGenesis bcs::to_bytes should success")
    }

    pub fn genesis_hash(&self) -> H256 {
        h256::sha3_256_of(self.encode().as_slice())
    }

    pub fn genesis_info(&self) -> GenesisInfo {
        GenesisInfo {
            genesis_package_hash: self.genesis_hash(),
            genesis_bin: self.encode(),
        }
    }

    pub fn save_to<P: AsRef<Path>>(&self, genesis_file: P) -> Result<()> {
        eprintln!("Save genesis to {:?}", genesis_file.as_ref());
        let mut file = File::create(genesis_file)?;
        let contents = bcs::to_bytes(&self)?;
        file.write_all(&contents)?;
        Ok(())
    }
}

impl KanariGenesisV2 {
    pub fn build(network: KanariNetwork) -> Result<Self> {
        let genesis_config = network.genesis_config;

        let stdlib = Self::load_stdlib(genesis_config.stdlib_version)?;

        let genesis_ctx = kanari_types::framework::genesis::GenesisContext::new(
            network.chain_id.id,
            genesis_config.sequencer_account,
            genesis_config.kanari_dao.multisign_bitcoin_address.clone(),
        );
        let moveos_genesis_ctx =
            moveos_types::moveos_std::genesis::GenesisContext::new(genesis_config.timestamp);
        let bitcoin_genesis_ctx = BitcoinGenesisContext::new(
            genesis_config.bitcoin_network,
            genesis_config.bitcoin_block_height,
            genesis_config.bitcoin_block_hash.into_address(),
            genesis_config.bitcoin_reorg_block_count,
            genesis_config.kanari_dao,
        );

        let bundles = stdlib.all_module_bundles()?;

        let genesis_tx = KanariTransaction::new_genesis_tx(
            KANARI_FRAMEWORK_ADDRESS.into(),
            network.chain_id.id,
            //merge all the module bundles into one
            MoveAction::ModuleBundle(
                bundles
                    .into_iter()
                    .flat_map(|(_, bundles)| bundles)
                    .collect(),
            ),
        );

        let mut genesis_moveos_tx = genesis_tx
            .clone()
            .into_moveos_transaction(ObjectMeta::genesis_root());

        let gas_parameter = if network.chain_id == BuiltinChainID::Main.chain_id() {
            FrameworksGasParameters::initial()
        } else if network.chain_id == BuiltinChainID::Test.chain_id() {
            FrameworksGasParameters::v1()
        } else {
            FrameworksGasParameters::latest()
        };

        let gas_config = gas_parameter.to_gas_schedule_config(network.chain_id);
        genesis_moveos_tx.ctx.add(genesis_ctx.clone())?;
        genesis_moveos_tx.ctx.add(moveos_genesis_ctx.clone())?;
        genesis_moveos_tx.ctx.add(bitcoin_genesis_ctx.clone())?;
        genesis_moveos_tx.ctx.add(gas_config.clone())?;

        let vm_config = MoveOSConfig::default();
        let (moveos_store, _temp_dir) = MoveOSStore::mock_moveos_store()?;
        let moveos = MoveOS::new(
            moveos_store,
            gas_parameter.all_natives(),
            vm_config,
            vec![],
            vec![],
        )?;
        let output = moveos.init_genesis(
            genesis_moveos_tx.clone(),
            genesis_config.genesis_objects.clone(),
        )?;

        Ok(Self {
            tx_output: output,
            initial_gas_config: gas_config,
            genesis_objects: genesis_config.genesis_objects,
            genesis_tx,
            genesis_moveos_tx,
        })
    }

    /// Load the genesis from binary or build the genesis if not exist
    pub fn load_or_build(network: KanariNetwork) -> Result<Self> {
        let genesis = if let Some(builtin_id) = network.chain_id.to_builtin() {
            load_genesis_from_binary(builtin_id)?
        } else {
            None
        };

        match genesis {
            Some(genesis) => Ok(genesis),
            None => {
                let genesis = Self::build(network)?;
                Ok(genesis)
            }
        }
    }

    pub fn genesis_tx(&self) -> KanariTransaction {
        self.genesis_tx.clone()
    }

    pub fn genesis_moveos_tx(&self) -> MoveOSTransaction {
        self.genesis_moveos_tx.clone()
    }

    pub fn genesis_hash(&self) -> H256 {
        h256::sha3_256_of(self.encode().as_slice())
    }

    pub fn genesis_info(&self) -> GenesisInfo {
        GenesisInfo {
            genesis_package_hash: self.genesis_hash(),
            genesis_bin: self.encode(),
        }
    }

    /// Load the genesis from the kanari db, if not exist, build and init the genesis
    pub fn load_or_init(network: KanariNetwork, kanari_db: &KanariDB) -> Result<Self> {
        let genesis_info = kanari_db.moveos_store.get_config_store().get_genesis()?;
        match genesis_info {
            Some(genesis_info_from_store) => {
                //if the genesis_info in the store we should check the genesis version between the store and the binary

                let genesis_from_binary = Self::load_or_build(network)?;
                let genesis_from_binary_v1 = KanariGenesis::from(genesis_from_binary.clone());

                let genesis_info_from_binary = genesis_from_binary.genesis_info();
                let genesis_info_from_binary_v1 = genesis_from_binary_v1.genesis_info();
                //Check both new and old genesis_package_hash
                if genesis_info_from_store.genesis_package_hash
                    != genesis_info_from_binary.genesis_package_hash
                    && genesis_info_from_store.genesis_package_hash
                        != genesis_info_from_binary_v1.genesis_package_hash
                {
                    return Err(GenesisError::GenesisVersionMismatch {
                        from_store: Box::new(genesis_info_from_store),
                        from_binary: Box::new(genesis_info_from_binary),
                    }
                    .into());
                }
                Self::decode(&genesis_info_from_store.genesis_bin)
            }
            None => {
                let genesis = Self::load_or_build(network)?;
                genesis.init_genesis(kanari_db)?;
                Ok(genesis)
            }
        }
    }

    pub fn init_genesis(&self, kanari_db: &KanariDB) -> Result<ObjectMeta> {
        ensure!(
            kanari_db
                .moveos_store
                .get_config_store()
                .get_genesis()?
                .is_none(),
            "Genesis already initialized"
        );

        //we load the gas parameter from genesis binary, avoid the code change affect the genesis result
        let genesis_gas_parameter = FrameworksGasParameters::load_from_gas_entries(
            self.initial_gas_config.max_gas_amount,
            self.initial_gas_config.entries.clone(),
        )?;
        let moveos = MoveOS::new(
            kanari_db.moveos_store.clone(),
            genesis_gas_parameter.all_natives(),
            MoveOSConfig::default(),
            vec![],
            vec![],
        )?;

        let genesis_raw_output =
            moveos.init_genesis(self.genesis_moveos_tx(), self.genesis_objects.clone())?;

        debug_assert!(
            genesis_raw_output == self.tx_output,
            "Genesis output mismatch"
        );

        // Save the genesis txs to sequencer
        let genesis_tx_order: u64 = 0;
        let moveos_genesis_context = self
            .genesis_moveos_tx()
            .ctx
            .get::<moveos_types::moveos_std::genesis::GenesisContext>()?
            .expect("Moveos Genesis context should exist");
        let mut tx_ledger_data = LedgerTxData::L2Tx(self.genesis_tx());
        let tx_hash = tx_ledger_data.tx_hash();
        // Init tx accumulator
        let genesis_tx_accumulator = MerkleAccumulator::new_with_info(
            AccumulatorInfo::default(),
            kanari_db.kanari_store.get_transaction_accumulator_store(),
        );
        let _genesis_accumulator_root = genesis_tx_accumulator.append(vec![tx_hash].as_slice())?;
        let genesis_accumulator_unsaved_nodes = genesis_tx_accumulator.pop_unsaved_nodes();

        let genesis_tx_accmulator_info = genesis_tx_accumulator.get_info();
        let ledger_tx = LedgerTransaction::build_ledger_transaction(
            tx_ledger_data,
            moveos_genesis_context.timestamp,
            genesis_tx_order,
            vec![],
            genesis_tx_accmulator_info.clone(),
        );
        let sequencer_info = SequencerInfo::new(genesis_tx_order, genesis_tx_accmulator_info);
        kanari_db.kanari_store.save_sequenced_tx(
            tx_hash,
            ledger_tx.clone(),
            sequencer_info,
            genesis_accumulator_unsaved_nodes,
            true,
        )?;
        genesis_tx_accumulator.clear_after_save();

        let tx_hash = self.genesis_tx().tx_hash();
        let (output, genesis_execution_info) = kanari_db
            .moveos_store
            .handle_tx_output(tx_hash, genesis_raw_output.clone())?;

        // Save genesis tx state change set
        let state_change_set_ext = StateChangeSetExt::new(
            output.changeset.clone(),
            self.genesis_moveos_tx().ctx.sequence_number,
        );
        kanari_db
            .kanari_store
            .save_state_change_set(genesis_tx_order, state_change_set_ext)?;

        // Save the genesis to indexer
        // 1. update indexer transaction
        let indexer_transaction = IndexerTransaction::new(
            ledger_tx.clone(),
            genesis_execution_info.clone(),
            self.genesis_moveos_tx().action,
            self.genesis_moveos_tx().ctx,
        )?;
        let transactions = vec![indexer_transaction];
        kanari_db.indexer_store.persist_transactions(transactions)?;

        // 2. update indexer event
        let events: Vec<_> = output
            .events
            .into_iter()
            .map(|event| {
                IndexerEvent::new(
                    event.clone(),
                    ledger_tx.clone(),
                    self.genesis_moveos_tx().ctx,
                )
            })
            .collect();
        kanari_db.indexer_store.persist_events(events)?;

        // 3. update indexer full object state, including object_states, utxos and inscriptions
        // indexer object state index generator
        let mut state_index_generator = IndexerObjectStatesIndexGenerator::default();
        let mut indexer_object_state_change_set = IndexerObjectStateChangeSet::default();

        for (_field_key, object_change) in genesis_raw_output.changeset.changes {
            handle_object_change(
                &mut state_index_generator,
                genesis_tx_order,
                &mut indexer_object_state_change_set,
                object_change,
            )?;
        }
        kanari_db
            .indexer_store
            .apply_object_states(indexer_object_state_change_set)?;

        let genesis_info = GenesisInfo::new(self.genesis_hash(), self.encode());
        kanari_db
            .moveos_store
            .get_config_store()
            .save_genesis(genesis_info)?;
        Ok(genesis_execution_info.root_metadata())
    }

    pub fn build_stdlib() -> Result<Stdlib> {
        framework_builder::stdlib_configs::build_stdlib(false)
    }

    pub fn load_stdlib(stdlib_version: StdlibVersion) -> Result<Stdlib> {
        framework_release::load_stdlib(stdlib_version)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        match KanariGenesis::decode(bytes) {
            Ok(genesis) => Ok(genesis.into()),
            // Parse with the old format first, and then try to parse with the new format if it fails
            Err(_e) => bcs::from_bytes(bytes).map_err(Into::into),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("KanariGenesisV2 bcs::to_bytes should success")
    }

    pub fn load_from<P: AsRef<Path>>(genesis_file: P) -> Result<Self> {
        let genesis_package = bcs::from_bytes(&std::fs::read(genesis_file)?)?;
        Ok(genesis_package)
    }

    pub fn save_to<P: AsRef<Path>>(&self, genesis_file: P) -> Result<()> {
        eprintln!("Save genesis to {:?}", genesis_file.as_ref());
        let mut file = File::create(genesis_file)?;
        let contents = bcs::to_bytes(&self)?;
        file.write_all(&contents)?;
        Ok(())
    }
}

pub(crate) fn path_in_crate<S>(relative: S) -> PathBuf
where
    S: AsRef<Path>,
{
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(relative);
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanari_config::KanariOpt;
    use kanari_db::KanariDB;
    use kanari_framework::KANARI_FRAMEWORK_ADDRESS;
    use kanari_types::bitcoin::multisign_account::MultisignAccountInfo;
    use kanari_types::bitcoin::network::BitcoinNetwork;
    use kanari_types::kanari_network::KanariNetwork;
    use move_core_types::identifier::Identifier;
    use move_core_types::language_storage::ModuleId;
    use move_core_types::resolver::{ModuleResolver, MoveResolver};
    use moveos_types::moveos_std::module_store::{ModuleStore, Package};
    use moveos_types::state::MoveStructType;
    use moveos_types::state_resolver::{RootObjectResolver, StateResolver};
    use state_resolver::StateReaderExt;
    use tracing::info;

    fn genesis_init_test_case(network: KanariNetwork, genesis: KanariGenesisV2) {
        info!(
            "genesis init test case for network: {:?}",
            network.chain_id.id
        );

        let opt = KanariOpt::new_with_temp_store().expect("create kanari opt failed");
        let kanari_db = KanariDB::init_with_mock_metrics_for_test(opt.store_config())
            .expect("init kanari db failed");

        let root = genesis.init_genesis(&kanari_db).unwrap();

        let resolver = RootObjectResolver::new(root, &kanari_db.moveos_store);
        let gas_parameter = FrameworksGasParameters::load_from_chain(&resolver)
            .expect("load gas parameter from chain failed");

        assert_eq!(
            genesis
                .initial_gas_config
                .entries
                .into_iter()
                .map(|entry| (entry.key, entry.val))
                .collect::<BTreeMap<_, _>>(),
            gas_parameter
                .to_gas_schedule_config(network.chain_id.clone())
                .entries
                .into_iter()
                .map(|entry| (entry.key, entry.val))
                .collect::<BTreeMap<_, _>>(),
        );

        let module_store_state = resolver.get_object(&ModuleStore::object_id()).unwrap();
        assert!(module_store_state.is_some());
        let module_store_obj = module_store_state
            .unwrap()
            .into_object::<ModuleStore>()
            .unwrap();
        assert!(
            module_store_obj.size > 0,
            "module store fields size should > 0"
        );

        let package_object_state = resolver
            .get_object(&Package::package_id(&KANARI_FRAMEWORK_ADDRESS))
            .unwrap();
        assert!(package_object_state.is_some());
        let package_obj = package_object_state
            .unwrap()
            .into_object::<Package>()
            .unwrap();
        assert!(package_obj.size > 0, "package fields size should > 0");

        let module = resolver
            .get_module(&ModuleId::new(
                KANARI_FRAMEWORK_ADDRESS,
                Identifier::new("genesis").unwrap(),
            ))
            .unwrap();
        assert!(module.is_some(), "genesis module should exist");

        let chain_id_state = resolver
            .get_object(&kanari_types::framework::chain_id::ChainID::chain_id_object_id())
            .unwrap();
        assert!(chain_id_state.is_some());
        let chain_id = chain_id_state
            .unwrap()
            .into_object::<kanari_types::framework::chain_id::ChainID>()
            .unwrap();
        assert_eq!(chain_id.value.id, network.chain_id.id);
        let bitcoin_network_state = resolver
            .get_object(&kanari_types::bitcoin::network::BitcoinNetwork::object_id())
            .unwrap();
        assert!(bitcoin_network_state.is_some());
        let bitcoin_network = bitcoin_network_state
            .unwrap()
            .into_object::<BitcoinNetwork>()
            .unwrap();
        assert_eq!(
            bitcoin_network.value.network,
            network.genesis_config.bitcoin_network
        );

        let kanari_dao_config = network.genesis_config.kanari_dao;
        let kanari_dao_address = kanari_dao_config
            .multisign_bitcoin_address
            .to_kanari_address();
        let kanari_dao_account = resolver.get_account(kanari_dao_address.into()).unwrap();
        assert!(kanari_dao_account.is_some());
        let multisign_account_info_data = resolver
            .get_resource(
                &kanari_dao_address.into(),
                &MultisignAccountInfo::struct_tag(),
            )
            .unwrap();
        assert!(multisign_account_info_data.is_some());
        let multisign_account_info: MultisignAccountInfo =
            bcs::from_bytes(&multisign_account_info_data.unwrap()).unwrap();
        assert!(
            multisign_account_info.multisign_bitcoin_address
                == kanari_dao_config.multisign_bitcoin_address
        );
    }

    #[tokio::test]
    async fn test_builtin_genesis_init() {
        let _ = tracing_subscriber::fmt::try_init();
        {
            let network: KanariNetwork = BuiltinChainID::Local.into();
            let genesis = KanariGenesisV2::load_or_build(network.clone()).unwrap();
            genesis_init_test_case(network, genesis);
        }
        {
            let network: KanariNetwork = BuiltinChainID::Dev.into();
            let genesis = KanariGenesisV2::load_or_build(network.clone()).unwrap();
            genesis_init_test_case(network, genesis);
        }
        {
            let network: KanariNetwork = BuiltinChainID::Test.into();
            let genesis = KanariGenesisV2::load_or_build(network.clone()).unwrap();
            genesis_init_test_case(network, genesis);
        }
        //We need to import the pre genesis state tree to init the mainnet genesis
        // {
        //     let network: KanariNetwork = BuiltinChainID::Main.into();
        //     let genesis = KanariGenesisV2::load_or_build(network.clone()).unwrap();
        //     genesis_init_test_case(network, genesis);
        // }
    }

    #[tokio::test]
    async fn test_custom_genesis_init() {
        let network: KanariNetwork =
            KanariNetwork::new(100.into(), BuiltinChainID::Test.genesis_config().clone());
        let genesis = KanariGenesisV2::build(network.clone()).unwrap();
        genesis_init_test_case(network, genesis);
    }

    #[test]
    fn test_genesis_load_from_binary() {
        assert!(
            load_genesis_from_binary(BuiltinChainID::Test)
                .unwrap()
                .is_some()
        );
        assert!(
            load_genesis_from_binary(BuiltinChainID::Main)
                .unwrap()
                .is_some()
        );
    }
}
