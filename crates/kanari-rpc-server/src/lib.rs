// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::metrics_server::{init_metrics, start_basic_prometheus_server};
use crate::server::btc_server::BtcServer;
use crate::server::kanari_server::KanariServer;
use crate::service::aggregate_service::AggregateService;
use crate::service::blocklist::{BlockListLayer, BlocklistConfig};
use crate::service::error::ErrorHandler;
use crate::service::metrics::ServiceMetrics;
use crate::service::rpc_service::RpcService;
use anyhow::{ensure, Error, Result};
use axum::http::{HeaderValue, Method};
use bitcoin_client::actor::client::BitcoinClientConfig;
use bitcoin_client::proxy::BitcoinClientProxy;
use coerce::actor::scheduler::timer::Timer;
use coerce::actor::{system::ActorSystem, IntoActor};
use jsonrpsee::RpcModule;
use moveos_eventbus::bus::EventBus;
use raw_store::errors::RawStoreError;
use kanari_config::da_config::derive_namespace_from_genesis;
use kanari_config::server_config::ServerConfig;
use kanari_config::settings::PROPOSER_CHECK_INTERVAL;
use kanari_config::{KanariOpt, ServerOpt};
use kanari_da::actor::server::DAServerActor;
use kanari_da::proxy::DAServerProxy;
use kanari_db::KanariDB;
use kanari_executor::actor::executor::ExecutorActor;
use kanari_executor::actor::reader_executor::ReaderExecutorActor;
use kanari_executor::proxy::ExecutorProxy;
use kanari_genesis::{KanariGenesis, KanariGenesisV2};
use kanari_indexer::actor::indexer::IndexerActor;
use kanari_indexer::actor::reader_indexer::IndexerReaderActor;
use kanari_indexer::proxy::IndexerProxy;
use kanari_notify::actor::NotifyActor;
use kanari_notify::subscription_handler::SubscriptionHandler;
use kanari_pipeline_processor::actor::processor::PipelineProcessorActor;
use kanari_pipeline_processor::proxy::PipelineProcessorProxy;
use kanari_proposer::actor::messages::ProposeBlock;
use kanari_proposer::actor::proposer::ProposerActor;
use kanari_relayer::actor::messages::RelayTick;
use kanari_relayer::actor::relayer::RelayerActor;
use kanari_rpc_api::api::KanariRpcModule;
use kanari_rpc_api::RpcError;
use kanari_sequencer::actor::sequencer::SequencerActor;
use kanari_sequencer::proxy::SequencerProxy;
use kanari_store::da_store::DAMetaStore;
use kanari_types::address::KanariAddress;
use kanari_types::error::{GenesisError, KanariError};
use kanari_types::kanari_network::BuiltinChainID;
use kanari_types::service_type::ServiceType;
use serde_json::json;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::{env, panic, process};
use tokio::signal;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

mod axum_router;
pub mod metrics_server;
pub mod server;
pub mod service;

/// This exit code means is that the server failed to start and required human intervention.
static R_EXIT_CODE_NEED_HELP: i32 = 120;

pub struct ServerHandle {
    shutdown_tx: Sender<()>,
    timers: Vec<Timer>,
    _opt: KanariOpt,
    _prometheus_registry: prometheus::Registry,
}

impl ServerHandle {
    fn stop(self) -> Result<()> {
        for timer in self.timers {
            timer.stop();
        }
        let _ = self.shutdown_tx.send(());
        Ok(())
    }
}

impl Debug for ServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerHandle").finish()
    }
}

#[derive(Debug, Default)]
pub struct Service {
    handle: Option<ServerHandle>,
}

impl Service {
    pub fn new() -> Self {
        Self { handle: None }
    }

    pub async fn start(&mut self, opt: KanariOpt, server_opt: ServerOpt) -> Result<()> {
        self.handle = Some(start_server(opt, server_opt).await?);
        Ok(())
    }

    pub fn stop(self) -> Result<()> {
        if let Some(handle) = self.handle {
            handle.stop()?
        }
        Ok(())
    }
}

pub struct RpcModuleBuilder {
    module: RpcModule<()>,
    // rpc_doc: Project,
}

impl Default for RpcModuleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcModuleBuilder {
    pub fn new() -> Self {
        Self {
            module: RpcModule::new(()),
            // rpc_doc: kanari_rpc_doc(env!("CARGO_PKG_VERSION")),
        }
    }

    pub fn register_module<M: KanariRpcModule>(&mut self, module: M) -> Result<()> {
        Ok(self.module.merge(module.rpc())?)
    }
}

// Start json-rpc server
pub async fn start_server(opt: KanariOpt, server_opt: ServerOpt) -> Result<ServerHandle> {
    let chain_name = opt.chain_id().chain_name();
    match run_start_server(opt, server_opt).await {
        Ok(server_handle) => Ok(server_handle),
        Err(e) => match e.downcast::<GenesisError>() {
            Ok(e) => {
                tracing::error!(
                    "{:?}, please clean your data dir. `kanari server clean -n {}` ",
                    e,
                    chain_name
                );
                std::process::exit(R_EXIT_CODE_NEED_HELP);
            }
            Err(e) => match e.downcast::<RawStoreError>() {
                Ok(e) => {
                    tracing::error!(
                        "{:?}, please clean your data dir. `kanari server clean -n {}` ",
                        e,
                        chain_name
                    );
                    std::process::exit(R_EXIT_CODE_NEED_HELP);
                }
                Err(e) => {
                    tracing::error!("{:?}, server start fail. ", e);
                    std::process::exit(R_EXIT_CODE_NEED_HELP);
                }
            },
        },
    }
}

// run json-rpc server
pub async fn run_start_server(opt: KanariOpt, server_opt: ServerOpt) -> Result<ServerHandle> {
    // We may call `start_server` multiple times in testing scenarios
    // tracing_subscriber can only be inited once.
    let _ = tracing_subscriber::fmt::try_init();

    // Exit the process when some thread panic
    // take_hook() returns the default hook in case when a custom one is not set
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        error!("Panic occurred:\n {} \n exit the process", panic_info);
        process::exit(1);
    }));

    let config = ServerConfig::new_with_port(opt.port());
    let actor_system = ActorSystem::global_system();

    // start prometheus server
    let prometheus_registry = start_basic_prometheus_server();
    // Initialize metrics before creating any stores
    init_metrics(&prometheus_registry);

    let (shutdown_tx, mut governor_rx): (broadcast::Sender<()>, broadcast::Receiver<()>) =
        broadcast::channel(16);

    // Init store
    let store_config = opt.store_config();

    let kanari_db = KanariDB::init(store_config, &prometheus_registry)?;
    let (kanari_store, moveos_store, indexer_store, indexer_reader) = (
        kanari_db.kanari_store.clone(),
        kanari_db.moveos_store.clone(),
        kanari_db.indexer_store.clone(),
        kanari_db.indexer_reader.clone(),
    );

    // Check for key pairs
    if server_opt.sequencer_keypair.is_none() || server_opt.proposer_keypair.is_none() {
        return Err(Error::from(
            KanariError::InvalidSequencerOrProposerOrRelayerKeyPair,
        ));
    }

    let sequencer_keypair = server_opt.sequencer_keypair.unwrap();
    let sequencer_account = sequencer_keypair.public().kanari_address()?;
    let sequencer_bitcoin_address = sequencer_keypair.public().bitcoin_address()?;

    let service_status = opt.service_status;

    let mut network = opt.network();
    if network.chain_id == BuiltinChainID::Local.chain_id() {
        // local chain use current active account as sequencer account
        let kanari_dao_bitcoin_address = network.mock_genesis_account(&sequencer_keypair)?;
        let kanari_dao_address = kanari_dao_bitcoin_address.to_kanari_address();
        println!("Kanari DAO address: {:?}", kanari_dao_address);
        println!("Kanari DAO Bitcoin address: {}", kanari_dao_bitcoin_address);
    } else {
        ensure!(
            network.genesis_config.sequencer_account == sequencer_bitcoin_address,
            "Sequencer({:?}) in genesis config is not equal to sequencer({:?}) in cli config",
            network.genesis_config.sequencer_account,
            sequencer_bitcoin_address
        );
    }

    let genesis = KanariGenesisV2::load_or_init(network.clone(), &kanari_db)?;

    let root = kanari_db
        .latest_root()?
        .ok_or_else(|| anyhow::anyhow!("No root object should exist after genesis init."))?;
    info!(
        "The latest Root object state root: {:?}, size: {}",
        root.state_root(),
        root.size()
    );

    let event_bus = EventBus::new();
    let subscription_handle = Arc::new(SubscriptionHandler::new(&prometheus_registry));
    let notify_actor = NotifyActor::new(event_bus.clone(), subscription_handle.clone());
    let notify_actor_ref = notify_actor
        .into_actor(Some("NotifyActor"), &actor_system)
        .await?;
    // let _notify_proxy = NotifyProxy::new(notify_actor_ref.clone().into());

    let executor_actor = ExecutorActor::new(
        root.clone(),
        moveos_store.clone(),
        kanari_store.clone(),
        &prometheus_registry,
        Some(notify_actor_ref.clone()),
    )?;

    let executor_actor_ref = executor_actor
        .into_actor(Some("Executor"), &actor_system)
        .await?;

    let reader_executor = ReaderExecutorActor::new(
        root.clone(),
        moveos_store.clone(),
        kanari_store.clone(),
        Some(notify_actor_ref.clone()),
    )?;

    let read_executor_ref = reader_executor
        .into_actor(Some("ReadExecutor"), &actor_system)
        .await?;

    let executor_proxy = ExecutorProxy::new(
        executor_actor_ref.clone().into(),
        read_executor_ref.clone().into(),
    );

    // Init sequencer
    info!("RPC Server sequencer address: {:?}", sequencer_account);
    let sequencer = SequencerActor::new(
        sequencer_keypair.copy(),
        kanari_store.clone(),
        service_status,
        &prometheus_registry,
        Some(notify_actor_ref.clone()),
    )?
    .into_actor(Some("Sequencer"), &actor_system)
    .await?;
    let sequencer_proxy = SequencerProxy::new(sequencer.into());

    // Init DA
    let genesis_v1 = KanariGenesis::from(genesis);
    let genesis_hash = genesis_v1.genesis_hash();
    let genesis_namespace = derive_namespace_from_genesis(genesis_hash);
    info!("DA genesis_namespace: {:?}", genesis_namespace);
    let last_tx_order = sequencer_proxy.get_sequencer_order().await?;
    let (da_issues, da_fixed) = kanari_store.try_repair_da_meta(
        last_tx_order,
        false,
        opt.da_config().da_min_block_to_submit,
        false,
        opt.service_status.is_sync_mode(),
    )?;
    info!("DA meta issues: {:?}, fixed: {:?}", da_issues, da_fixed);
    let da_config = opt.da_config().clone();
    let da_proxy = DAServerProxy::new(
        DAServerActor::new(
            da_config,
            sequencer_keypair.copy(),
            kanari_store.clone(),
            genesis_namespace,
            shutdown_tx.subscribe(),
        )
        .await?
        .into_actor(Some("DAServer"), &actor_system)
        .await?
        .into(),
    );

    // Init proposer
    let proposer_keypair = server_opt.proposer_keypair.unwrap();
    let proposer_account: KanariAddress = proposer_keypair.public().kanari_address()?;
    info!("RPC Server proposer address: {:?}", proposer_account);
    let proposer = ProposerActor::new(
        proposer_keypair,
        moveos_store.clone(),
        kanari_store,
        &prometheus_registry,
        opt.proposer.clone(),
    )?
    .into_actor(Some("Proposer"), &actor_system)
    .await?;
    let block_propose_duration_in_seconds: u64 =
        opt.proposer.interval.unwrap_or(PROPOSER_CHECK_INTERVAL);
    let mut timers = vec![];
    let proposer_timer = Timer::start(
        proposer,
        Duration::from_secs(block_propose_duration_in_seconds),
        ProposeBlock {},
    );
    timers.push(proposer_timer);

    // Init indexer
    let indexer_executor = IndexerActor::new(
        root,
        indexer_store,
        moveos_store,
        Some(notify_actor_ref.clone()),
    )?
    .into_actor(Some("Indexer"), &actor_system)
    .await?;
    let indexer_reader_executor = IndexerReaderActor::new(indexer_reader)?
        .into_actor(Some("IndexerReader"), &actor_system)
        .await?;
    let indexer_proxy = IndexerProxy::new(indexer_executor.into(), indexer_reader_executor.into());
    let bitcoin_relayer_config = opt.bitcoin_relayer_config();
    let bitcoin_client_config = bitcoin_relayer_config
        .as_ref()
        .map(|config| BitcoinClientConfig {
            btc_rpc_url: config.btc_rpc_url.clone(),
            btc_rpc_user_name: config.btc_rpc_user_name.clone(),
            btc_rpc_password: config.btc_rpc_password.clone(),
            local_block_store_dir: Some(config.btc_reorg_aware_block_store_dir.clone()), // this client will be used for startup processing, may need reorg blocks
        });
    let bitcoin_client_proxy = if service_status.is_active() && bitcoin_client_config.is_some() {
        let bitcoin_client = bitcoin_client_config.unwrap().build()?;
        let bitcoin_client_actor_ref = bitcoin_client
            .into_actor(Some("bitcoin_client_for_rpc_service"), &actor_system)
            .await?;
        let bitcoin_client_proxy = BitcoinClientProxy::new(bitcoin_client_actor_ref.into());
        Some(bitcoin_client_proxy)
    } else {
        None
    };

    let mut processor = PipelineProcessorActor::new(
        executor_proxy.clone(),
        sequencer_proxy.clone(),
        da_proxy.clone(),
        indexer_proxy.clone(),
        service_status,
        &prometheus_registry,
        Some(notify_actor_ref.clone()),
        kanari_db,
        bitcoin_client_proxy.clone(),
    );

    // Only process sequenced tx on startup when service is active
    if service_status.is_active() {
        processor.process_sequenced_tx_on_startup().await?;
    }
    let processor_actor = processor
        .into_actor(Some("PipelineProcessor"), &actor_system)
        .await?;
    let processor_proxy = PipelineProcessorProxy::new(processor_actor.into());

    let ethereum_relayer_config = opt.ethereum_relayer_config();

    if service_status.is_active()
        && (ethereum_relayer_config.is_some() || bitcoin_relayer_config.is_some())
    {
        let relayer = RelayerActor::new(
            executor_proxy.clone(),
            processor_proxy.clone(),
            ethereum_relayer_config,
            bitcoin_relayer_config.clone(),
            Some(notify_actor_ref),
        )
        .await?
        .into_actor(Some("Relayer"), &actor_system)
        .await?;
        let relay_tick_in_seconds: u64 = 1;
        let relayer_timer = Timer::start(
            relayer,
            Duration::from_secs(relay_tick_in_seconds),
            RelayTick {},
        );
        timers.push(relayer_timer);
    }

    let rpc_service = RpcService::new(
        network.chain_id.id,
        network.genesis_config.bitcoin_network,
        executor_proxy,
        sequencer_proxy,
        indexer_proxy,
        processor_proxy,
        bitcoin_client_proxy,
        da_proxy,
        subscription_handle.clone(),
        None,
    );
    let aggregate_service = AggregateService::new(rpc_service.clone());

    let acl = match env::var("ACCESS_CONTROL_ALLOW_ORIGIN") {
        Ok(value) => {
            let allow_hosts = value
                .split(',')
                .map(HeaderValue::from_str)
                .collect::<Result<Vec<_>, _>>()?;
            AllowOrigin::list(allow_hosts)
        }
        _ => AllowOrigin::any(),
    };
    info!(?acl);

    // init cors
    let cors: CorsLayer = CorsLayer::new()
        // Allow `POST` when accessing the resource
        .allow_methods([Method::POST])
        // Allow requests from any origin
        .allow_origin(acl)
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let traffic_burst_size: u32;
    let traffic_per_second: f64;

    if network.chain_id != BuiltinChainID::Local.chain_id() {
        traffic_burst_size = opt.traffic_burst_size.unwrap_or(200);
        traffic_per_second = opt.traffic_per_second.unwrap_or(0.1f64);
    } else {
        traffic_burst_size = opt.traffic_burst_size.unwrap_or(5000);
        traffic_per_second = opt.traffic_per_second.unwrap_or(0.001f64);
    };

    // init limit
    // Allow bursts with up to x requests per IP address
    // and replenishes one element every x seconds
    // We Box it because Axum 0.6 requires all Layers to be Clone
    // and thus we need a static reference to it
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(SmartIpKeyExtractor)
            .use_headers()
            .per_millisecond((traffic_per_second * 1000f64) as u64)
            .burst_size(traffic_burst_size)
            .use_headers()
            .error_handler(move |error1| ErrorHandler::default().0(error1))
            .finish()
            .unwrap(),
    );

    let governor_limiter = governor_conf.limiter().clone();
    let interval = Duration::from_secs(60);

    // a separate background task to clean up
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        loop {
            if governor_rx.try_recv().is_ok() {
                info!("Background thread received cancel signal, stopping.");
                break;
            }
            tick.tick().await;
            tracing::info!("rate limiting storage size: {}", governor_limiter.len());
            governor_limiter.retain_recent();
        }
    });

    let blocklist_config = Arc::new(BlocklistConfig::default());

    let middleware = tower::ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(BlockListLayer {
            config: blocklist_config,
        })
        .layer(GovernorLayer {
            config: governor_conf,
        });

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    let mut rpc_module_builder = RpcModuleBuilder::new();
    rpc_module_builder.register_module(KanariServer::new(
        rpc_service.clone(),
        aggregate_service.clone(),
    ))?;
    rpc_module_builder.register_module(BtcServer::new(rpc_service.clone()).await?)?;
    rpc_module_builder
        .module
        .register_method("rpc.discover", move |_, _, _| {
            Ok::<kanari_open_rpc::Project, RpcError>(
                kanari_open_rpc_spec_builder::build_kanari_rpc_spec(),
            )
        })?;

    let methods_names = rpc_module_builder.module.method_names().collect::<Vec<_>>();

    let ser = axum_router::JsonRpcService::new(
        rpc_module_builder.module.clone().into(),
        ServiceMetrics::new(&prometheus_registry, &methods_names),
        subscription_handle,
    );

    let mut router = axum::Router::new();
    match opt.service_type {
        ServiceType::Both => {
            router = router
                .route(
                    "/",
                    axum::routing::post(crate::axum_router::json_rpc_handler),
                )
                .route(
                    "/",
                    axum::routing::get(crate::axum_router::ws::ws_json_rpc_upgrade),
                )
                .route(
                    "/subscribe",
                    axum::routing::get(crate::axum_router::ws::ws_json_rpc_upgrade),
                )
                .route(
                    "/subscribe/sse/events",
                    axum::routing::get(crate::axum_router::sse_events_handler),
                )
                .route(
                    "/subscribe/sse/transactions",
                    axum::routing::get(crate::axum_router::sse_transactions_handler),
                );
        }
        ServiceType::Http => {
            router = router
                .route("/", axum::routing::post(axum_router::json_rpc_handler))
                .route(
                    "/subscribe/sse/events",
                    axum::routing::get(crate::axum_router::sse_events_handler),
                )
                .route(
                    "/subscribe/sse/transactions",
                    axum::routing::get(crate::axum_router::sse_transactions_handler),
                );
        }
        ServiceType::WebSocket => {
            router = router
                .route(
                    "/",
                    axum::routing::get(crate::axum_router::ws::ws_json_rpc_upgrade),
                )
                .route(
                    "/subscribe",
                    axum::routing::get(crate::axum_router::ws::ws_json_rpc_upgrade),
                )
        }
    }

    let app = router.with_state(ser).layer(middleware);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let addr = listener.local_addr()?;

    let mut rpc_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            tokio::select! {
            _ = shutdown_signal() => {},
            _ = rpc_rx.recv() => {
                info!("shutdown signal received, starting graceful shutdown");
                },
            }
        })
        .await
        .unwrap();
    });

    info!("JSON-RPC HTTP Server start listening {:?}", addr);
    info!("Available JSON-RPC methods : {:?}", methods_names);

    Ok(ServerHandle {
        shutdown_tx,
        timers,
        _opt: opt,
        _prometheus_registry: prometheus_registry,
    })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    info!("Terminate signal received");
}

fn _build_rpc_api<M: Send + Sync + 'static>(mut rpc_module: RpcModule<M>) -> RpcModule<M> {
    let mut available_methods = rpc_module.method_names().collect::<Vec<_>>();
    available_methods.sort();

    rpc_module
        .register_method("rpc_methods", move |_, _, _| {
            Ok::<serde_json::Value, RpcError>(json!({
                "methods": available_methods,
            }))
        })
        .expect("infallible all other methods have their own address space");

    rpc_module
}
