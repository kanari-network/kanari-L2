// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use coerce::actor::{IntoActor, system::ActorSystem};
use kanari_config::KanariOpt;
use kanari_db::KanariDB;
use kanari_genesis::KanariGenesisV2;
use kanari_sequencer::{actor::sequencer::SequencerActor, proxy::SequencerProxy};
use kanari_types::{
    crypto::KanariKeyPair,
    service_status::ServiceStatus,
    transaction::{KanariTransaction, LedgerTxData},
};
use metrics::RegistryService;
use prometheus::Registry;
use raw_store::StoreInstance;
use raw_store::metrics::DBMetrics;

fn init_kanari_db(opt: &KanariOpt, registry: &Registry) -> Result<KanariDB> {
    DBMetrics::init(registry);
    let store_instance = KanariDB::generate_store_instance(opt.store_config(), registry)?;
    init_kanari_db_with_instance(opt, store_instance, registry)
}

fn init_kanari_db_with_instance(
    opt: &KanariOpt,
    instance: StoreInstance,
    registry: &Registry,
) -> Result<KanariDB> {
    let kanari_db = KanariDB::init_with_instance(opt.store_config(), instance, registry)?;
    let network = opt.network();
    let _genesis = KanariGenesisV2::load_or_init(network, &kanari_db)?;
    Ok(kanari_db)
}

#[tokio::test]
async fn test_sequencer() -> Result<()> {
    let opt = KanariOpt::new_with_temp_store()?;
    let mut last_tx_order = 0;
    let registry_service = RegistryService::default();
    {
        let store_instance = KanariDB::generate_store_instance(
            opt.store_config(),
            &registry_service.default_registry(),
        )?;
        let kanari_db = init_kanari_db_with_instance(
            &opt,
            store_instance.clone(),
            &registry_service.default_registry(),
        )?;
        let sequencer_key = KanariKeyPair::generate_secp256k1();
        let mut sequencer = SequencerActor::new(
            sequencer_key,
            kanari_db.kanari_store,
            ServiceStatus::Active,
            &registry_service.default_registry(),
            None,
        )?;
        assert_eq!(sequencer.last_order(), last_tx_order);
        for _ in 0..10 {
            let tx_data = LedgerTxData::L2Tx(KanariTransaction::mock());
            let ledger_tx = sequencer.sequence(tx_data)?;
            assert_eq!(ledger_tx.sequence_info.tx_order, last_tx_order + 1);
            last_tx_order = ledger_tx.sequence_info.tx_order;
        }
        assert_eq!(sequencer.last_order(), last_tx_order);
    }
    // load from db again
    {
        // To avoid AlreadyReg for re init the same db
        let new_registry = prometheus::Registry::new();
        let kanari_db = KanariDB::init(opt.store_config(), &new_registry)?;
        let sequencer_key = KanariKeyPair::generate_secp256k1();
        let mut sequencer = SequencerActor::new(
            sequencer_key,
            kanari_db.kanari_store,
            ServiceStatus::Active,
            &new_registry,
            None,
        )?;
        assert_eq!(sequencer.last_order(), last_tx_order);
        let tx_data = LedgerTxData::L2Tx(KanariTransaction::mock());
        let ledger_tx = sequencer.sequence(tx_data)?;
        assert_eq!(ledger_tx.sequence_info.tx_order, last_tx_order + 1);
    }
    Ok(())
}

// test concurrent
// Build a sequencer actor and sequence transactions concurrently
#[tokio::test(flavor = "multi_thread", worker_threads = 5)]
async fn test_sequencer_concurrent() -> Result<()> {
    let opt = KanariOpt::new_with_temp_store()?;
    let registry_service = RegistryService::default();
    let kanari_db = init_kanari_db(&opt, &registry_service.default_registry())?;
    let sequencer_key = KanariKeyPair::generate_secp256k1();

    let actor_system = ActorSystem::global_system();

    let sequencer = SequencerActor::new(
        sequencer_key,
        kanari_db.kanari_store,
        ServiceStatus::Active,
        &registry_service.default_registry(),
        None,
    )?
    .into_actor(Some("Sequencer"), &actor_system)
    .await?;
    let sequencer_proxy = SequencerProxy::new(sequencer.into());

    // start n thread to sequence
    let n = 10;
    let mut handles = vec![];
    for _ in 0..n {
        let sequencer_proxy = sequencer_proxy.clone();
        //Use tokio to spawn a new async task
        let handle = tokio::task::spawn(async move {
            for _ in 0..n {
                let tx_data = LedgerTxData::L2Tx(KanariTransaction::mock());
                let _ = sequencer_proxy.sequence_transaction(tx_data).await.unwrap();
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    let sequencer_order = sequencer_proxy.get_sequencer_order().await?;
    assert_eq!(sequencer_order, n * n);

    Ok(())
}
