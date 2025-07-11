// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::errors::IndexerError;
use crate::metrics::IndexerReaderMetrics;
use crate::models::events::StoredEvent;
use crate::models::fields::StoredField;
use crate::models::states::{StoredObjectStateInfo, StoredStateID};
use crate::models::transactions::StoredTransaction;
use crate::schema::{events, transactions};
use crate::utils::escape_sql_string;
use crate::{
    DEFAULT_BUSY_TIMEOUT, INDEXER_EVENTS_TABLE_NAME, INDEXER_FIELDS_TABLE_NAME,
    INDEXER_OBJECT_STATE_INSCRIPTIONS_TABLE_NAME, INDEXER_OBJECT_STATE_UTXOS_TABLE_NAME,
    INDEXER_OBJECT_STATES_TABLE_NAME, INDEXER_TRANSACTIONS_TABLE_NAME, IndexerResult,
    IndexerStoreMeta, IndexerTableName, SqliteConnectionConfig, SqliteConnectionPoolConfig,
    SqlitePoolConnection,
};
use anyhow::{Result, anyhow};
use diesel::{
    Connection, ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection, r2d2::ConnectionManager,
};
use function_name::named;
use kanari_types::indexer::event::{EventFilter, IndexerEvent, IndexerEventID};
use kanari_types::indexer::field::{FieldFilter, IndexerField};
use kanari_types::indexer::state::{IndexerStateID, ObjectStateFilter, ObjectStateType};
use kanari_types::indexer::transaction::{IndexerTransaction, TransactionFilter};
use move_core_types::language_storage::StructTag;
use moveos_types::moveos_std::object::ObjectID;
use prometheus::Registry;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::time::timeout;

pub const DEFAULT_QUERY_TIMEOUT: u64 = 60; // second

pub const TX_ORDER_STR: &str = "tx_order";
pub const TX_HASH_STR: &str = "tx_hash";
pub const TX_SENDER_STR: &str = "sender";
pub const CREATED_AT_STR: &str = "created_at";
pub const OBJECT_ID_STR: &str = "id";

pub const EVENT_HANDLE_ID_STR: &str = "event_handle_id";
pub const EVENT_INDEX_STR: &str = "event_index";
pub const EVENT_SEQ_STR: &str = "event_seq";
pub const EVENT_TYPE_STR: &str = "event_type";

pub const STATE_OBJECT_ID_STR: &str = "id";
pub const STATE_INDEX_STR: &str = "state_index";
pub const STATE_OBJECT_TYPE_STR: &str = "object_type";
pub const STATE_OWNER_STR: &str = "owner";

pub const PARENT_OBJECT_ID_STR: &str = "parent_id";
pub const SORT_KEY_STR: &str = "sort_key";

#[derive(Clone)]
pub struct InnerIndexerReader {
    pub(crate) pool: crate::SqliteConnectionPool,
}

impl InnerIndexerReader {
    pub fn new_with_config<T: Into<String>>(
        db_url: T,
        config: SqliteConnectionPoolConfig,
    ) -> Result<Self> {
        let manager = ConnectionManager::<SqliteConnection>::new(db_url);

        let locker = Arc::new(RwLock::new(0));
        let connection_config = SqliteConnectionConfig {
            read_only: true,
            enable_wal: true,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
            locker,
        };

        let pool = diesel::r2d2::Pool::builder()
            .max_size(config.pool_size)
            .connection_timeout(config.connection_timeout)
            .connection_customizer(Box::new(connection_config))
            .build(manager)
            .map_err(|e| anyhow!("Failed to initialize connection pool. Error: {:?}. If Error is None, please check whether the configured pool size (currently {}) exceeds the maximum number of connections allowed by the database.", e, config.pool_size))?;

        Ok(Self { pool })
    }

    pub fn get_connection(&self) -> Result<SqlitePoolConnection, IndexerError> {
        self.pool.get().map_err(|e| {
            IndexerError::SqlitePoolConnectionError(format!(
                "Failed to get connection from SQLite connection pool with error: {:?}",
                e
            ))
        })
    }

    pub fn run_query<T, E, F>(&self, query: F) -> Result<T, IndexerError>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T, E>,
        E: From<diesel::result::Error> + std::error::Error,
    {
        let mut connection = self.get_connection()?;
        connection
            .deref_mut()
            .transaction(query)
            .map_err(|e| IndexerError::SQLiteReadError(e.to_string()))
    }

    pub fn run_query_with_timeout<T, E, F>(&self, query: F) -> Result<T, IndexerError>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T, E> + Send + 'static,
        E: From<diesel::result::Error> + std::error::Error + Send,
        T: Send + 'static,
    {
        // default query time out in second
        let timeout_duration = Duration::from_secs(DEFAULT_QUERY_TIMEOUT);
        let mut connection = self.get_connection()?;

        tokio::task::block_in_place(|| {
            Handle::current().block_on(async move {
                timeout(
                    timeout_duration,
                    tokio::task::spawn_blocking(move || {
                        connection
                            .deref_mut()
                            .transaction(query)
                            .map_err(|e| IndexerError::SQLiteReadError(e.to_string()))
                    }),
                )
                .await
                .map_err(|e| IndexerError::SQLiteAsyncReadError(e.to_string()))??
            })
        })
    }
}

#[derive(Clone)]
pub struct IndexerReader {
    pub(crate) inner_indexer_reader_mapping: HashMap<String, InnerIndexerReader>,
    metrics: Arc<IndexerReaderMetrics>,
}

impl IndexerReader {
    pub fn new(db_path: PathBuf, registry: &Registry) -> Result<Self> {
        let config = SqliteConnectionPoolConfig::pool_config(true);
        Self::new_with_config(db_path, config, registry)
    }

    pub fn new_with_config(
        db_path: PathBuf,
        config: SqliteConnectionPoolConfig,
        registry: &Registry,
    ) -> Result<Self> {
        let tables = IndexerStoreMeta::get_indexer_table_names().to_vec();

        let mut inner_indexer_reader_mapping = HashMap::<String, InnerIndexerReader>::new();
        for table in tables {
            let indexer_db_url = db_path
                .clone()
                .join(table)
                .to_str()
                .ok_or(anyhow::anyhow!("Invalid indexer db path"))?
                .to_string();

            let inner_indexer_reader = InnerIndexerReader::new_with_config(indexer_db_url, config)?;
            inner_indexer_reader_mapping.insert(table.to_string(), inner_indexer_reader);
        }

        Ok(IndexerReader {
            inner_indexer_reader_mapping,
            metrics: Arc::new(IndexerReaderMetrics::new(registry)),
        })
    }

    pub fn get_inner_indexer_reader(&self, table_name: &str) -> Result<InnerIndexerReader> {
        Ok(self
            .inner_indexer_reader_mapping
            .get(table_name)
            .ok_or(anyhow::anyhow!("Inner indexer reader not exist"))?
            .clone())
    }

    #[named]
    pub fn query_transactions_with_filter(
        &self,
        filter: TransactionFilter,
        cursor: Option<u64>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IndexerTransaction>> {
        let start = Instant::now();
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .indexer_reader_query_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let tx_order = if let Some(cursor) = cursor {
            cursor as i64
        } else if descending_order {
            let max_tx_order: i64 = self
                .get_inner_indexer_reader(INDEXER_TRANSACTIONS_TABLE_NAME)?
                .run_query_with_timeout(|conn| {
                    transactions::dsl::transactions
                        .select(transactions::tx_order)
                        .order_by(transactions::tx_order.desc())
                        .first::<i64>(conn)
                })?;
            max_tx_order + 1
        } else {
            -1
        };

        let main_where_clause = match filter {
            TransactionFilter::Sender(sender) => {
                format!("{TX_SENDER_STR} = \"{}\"", sender.to_hex_literal())
            }
            TransactionFilter::TxHashes(tx_hashes) => {
                let in_tx_hash_str: String = tx_hashes
                    .iter()
                    .map(|tx_hash| format!("\"{:?}\"", tx_hash))
                    .collect::<Vec<String>>()
                    .join(",");
                format!("{TX_HASH_STR} in ({})", in_tx_hash_str)
            }
            TransactionFilter::TimeRange {
                start_time,
                end_time,
            } => {
                format!(
                    "({CREATED_AT_STR} >= {} AND {CREATED_AT_STR} < {})",
                    start_time, end_time
                )
            }
            TransactionFilter::TxOrderRange {
                from_order,
                to_order,
            } => {
                format!(
                    "({TX_ORDER_STR} >= {} AND {TX_ORDER_STR} < {})",
                    from_order, to_order
                )
            }
            TransactionFilter::All => {
                return Err(IndexerError::NotSupportedError(
                    "Not implemented".to_string(),
                ));
            }
        };

        let cursor_clause = if descending_order {
            format!("AND ({TX_ORDER_STR} < {})", tx_order)
        } else {
            format!("AND ({TX_ORDER_STR} > {})", tx_order)
        };
        let order_clause = if descending_order {
            format!("{TX_ORDER_STR} DESC")
        } else {
            format!("{TX_ORDER_STR} ASC")
        };

        let query = format!(
            "
                SELECT * FROM transactions \
                WHERE {} {} \
                ORDER BY {} \
                LIMIT {}
            ",
            main_where_clause, cursor_clause, order_clause, limit,
        );

        tracing::debug!("Query transactions: {}", query);
        let stored_transactions = self
            .get_inner_indexer_reader(INDEXER_TRANSACTIONS_TABLE_NAME)?
            .run_query_with_timeout(|conn| {
                diesel::sql_query(query).load::<StoredTransaction>(conn)
            })?;

        let result = stored_transactions
            .into_iter()
            .map(IndexerTransaction::try_from)
            .collect::<Result<Vec<_>>>()
            .map_err(|e| {
                IndexerError::SQLiteReadError(format!("Cast indexer transactions failed: {:?}", e))
            })?;
        tracing::debug!("Query transactions time elapsed: {:?}", start.elapsed());

        Ok(result)
    }

    #[named]
    pub fn query_events_with_filter(
        &self,
        filter: EventFilter,
        cursor: Option<IndexerEventID>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IndexerEvent>> {
        let start = Instant::now();
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .indexer_reader_query_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let (tx_order, event_index) = if let Some(cursor) = cursor {
            let IndexerEventID {
                tx_order,
                event_index,
            } = cursor;
            (tx_order as i64, event_index as i64)
        } else if descending_order {
            let (max_tx_order, event_index): (i64, i64) = self
                .get_inner_indexer_reader(INDEXER_EVENTS_TABLE_NAME)?
                .run_query_with_timeout(|conn| {
                    events::dsl::events
                        .select((events::tx_order, events::event_index))
                        .order_by((events::tx_order.desc(), events::event_index.desc()))
                        .first::<(i64, i64)>(conn)
                })?;
            (max_tx_order, event_index + 1)
        } else {
            (-1, 0)
        };

        let main_where_clause = match filter {
            EventFilter::EventTypeWithSender { event_type, sender } => {
                format!(
                    "{TX_SENDER_STR} = \"{}\" AND {EVENT_TYPE_STR} = \"{}\"",
                    sender.to_hex_literal(),
                    event_type
                )
            }
            EventFilter::EventType(event_type) => {
                format!("{EVENT_TYPE_STR} = \"{}\"", event_type)
            }
            EventFilter::EventHandleWithSender {
                event_handle_id,
                sender,
            } => {
                format!(
                    "{TX_SENDER_STR} = \"{}\" AND {EVENT_HANDLE_ID_STR} = \"{}\"",
                    sender.to_hex_literal(),
                    event_handle_id
                )
            }
            EventFilter::EventHandle(event_handle_id) => {
                format!("{EVENT_HANDLE_ID_STR} = \"{}\"", event_handle_id)
            }
            EventFilter::Sender(sender) => {
                format!("{TX_SENDER_STR} = \"{}\"", sender.to_hex_literal())
            }
            EventFilter::TxHash(tx_hash) => {
                let tx_hash_str = format!("{:?}", tx_hash);
                format!("{TX_HASH_STR} = \"{}\"", tx_hash_str)
            }
            EventFilter::TimeRange {
                start_time,
                end_time,
            } => {
                format!(
                    "({CREATED_AT_STR} >= {} AND {CREATED_AT_STR} < {})",
                    start_time, end_time
                )
            }
            EventFilter::TxOrderRange {
                from_order,
                to_order,
            } => {
                format!(
                    "({TX_ORDER_STR} >= {} AND {TX_ORDER_STR} < {})",
                    from_order, to_order
                )
            }
            EventFilter::All => {
                return Err(IndexerError::NotSupportedError(
                    "Not implemented".to_string(),
                ));
            }
        };

        let cursor_clause = if descending_order {
            format!(
                "AND ({TX_ORDER_STR} < {} OR ({TX_ORDER_STR} = {} AND {EVENT_INDEX_STR} < {}))",
                tx_order, tx_order, event_index
            )
        } else {
            format!(
                "AND ({TX_ORDER_STR} > {} OR ({TX_ORDER_STR} = {} AND {EVENT_INDEX_STR} > {}))",
                tx_order, tx_order, event_index
            )
        };
        let order_clause = if descending_order {
            format!("{TX_ORDER_STR} DESC, {EVENT_INDEX_STR} DESC")
        } else {
            format!("{TX_ORDER_STR} ASC, {EVENT_INDEX_STR} ASC")
        };

        let query = format!(
            "
                SELECT * FROM events \
                WHERE {} {} \
                ORDER BY {} \
                LIMIT {}
            ",
            main_where_clause, cursor_clause, order_clause, limit,
        );

        tracing::debug!("Query events: {}", query);
        let stored_events = self
            .get_inner_indexer_reader(INDEXER_EVENTS_TABLE_NAME)?
            .run_query_with_timeout(|conn| diesel::sql_query(query).load::<StoredEvent>(conn))?;

        let result = stored_events
            .into_iter()
            .map(|ev| ev.try_into_indexer_event())
            .collect::<Result<Vec<_>>>()
            .map_err(|e| {
                IndexerError::SQLiteReadError(format!("Cast indexer events failed: {:?}", e))
            })?;
        tracing::debug!("Query events time elapsed: {:?}", start.elapsed());

        Ok(result)
    }

    fn query_stored_object_state_infos_with_filter(
        &self,
        filter: ObjectStateFilter,
        cursor: Option<IndexerStateID>,
        limit: usize,
        descending_order: bool,
        state_type: ObjectStateType,
    ) -> IndexerResult<Vec<StoredObjectStateInfo>> {
        let start = Instant::now();
        let (tx_order, state_index) = if let Some(cursor) = cursor {
            let IndexerStateID {
                tx_order,
                state_index,
            } = cursor;
            (tx_order as i64, state_index as i64)
        } else if descending_order {
            let last_state_id = self.query_last_indexer_state_id(state_type.clone())?;
            match last_state_id {
                Some((max_tx_order, state_index)) => (max_tx_order, state_index + 1),
                None => (0, 0),
            }
        } else {
            (-1, 0)
        };

        let table_name = get_table_name_by_state_type(state_type.clone());
        // Avoid to use "select *". Specify the columns to use.
        let select_clause = format!(
            "SELECT {STATE_OBJECT_ID_STR},{TX_ORDER_STR},{STATE_INDEX_STR} FROM {}",
            table_name
        );

        let main_where_clause = match filter {
            ObjectStateFilter::ObjectTypeWithOwner {
                object_type,
                owner,
                filter_out,
            } => {
                match state_type {
                    ObjectStateType::ObjectState => {
                        let object_query = if filter_out {
                            not_object_type_query(&object_type)
                        } else {
                            object_type_query(&object_type)
                        };
                        format!(
                            "{STATE_OWNER_STR} = \"{}\" AND {}",
                            owner.to_hex_literal(),
                            object_query
                        )
                    }
                    _ => {
                        // Ignore object_type param for utxo and inscription query
                        format!("{STATE_OWNER_STR} = \"{}\"", owner.to_hex_literal(),)
                    }
                }
            }
            ObjectStateFilter::ObjectType(object_type) => {
                match state_type {
                    ObjectStateType::ObjectState => object_type_query(&object_type),
                    // Ignore object_type param for utxo and inscription query
                    _ => " ".to_string(),
                }
            }

            ObjectStateFilter::Owner(owner) => {
                format!("{STATE_OWNER_STR} = \"{}\"", owner.to_hex_literal())
            }
            ObjectStateFilter::ObjectId(object_ids) => {
                let object_ids_str = object_ids
                    .into_iter()
                    .map(|obj_id| format!("\"{}\"", obj_id))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{OBJECT_ID_STR} IN ({object_ids_str})")
            }
        };

        let has_main_where_clause = main_where_clause.ne(" ");
        let cursor_clause_start = if has_main_where_clause { "AND" } else { " " };
        let cursor_clause = if descending_order {
            format!(
                "{} ({TX_ORDER_STR} < {} OR ({TX_ORDER_STR} = {} AND {STATE_INDEX_STR} < {}))",
                cursor_clause_start, tx_order, tx_order, state_index
            )
        } else {
            format!(
                "{} ({TX_ORDER_STR} > {} OR ({TX_ORDER_STR} = {} AND {STATE_INDEX_STR} > {}))",
                cursor_clause_start, tx_order, tx_order, state_index
            )
        };
        let order_clause = if descending_order {
            format!("{TX_ORDER_STR} DESC, {STATE_INDEX_STR} DESC")
        } else {
            format!("{TX_ORDER_STR} ASC, {STATE_INDEX_STR} ASC")
        };

        let query = format!(
            "
                {} \
                WHERE {} {} \
                ORDER BY {} \
                LIMIT {}
            ",
            select_clause, main_where_clause, cursor_clause, order_clause, limit,
        );

        tracing::debug!("Query object states: {}", query);
        let stored_object_state_infos = self
            .get_inner_indexer_reader(table_name)?
            .run_query_with_timeout(|conn| {
                diesel::sql_query(query).load::<StoredObjectStateInfo>(conn)
            })?;

        tracing::debug!("Query object states time elapsed: {:?}", start.elapsed());
        Ok(stored_object_state_infos)
    }

    #[named]
    pub fn query_object_ids_with_filter(
        &self,
        filter: ObjectStateFilter,
        cursor: Option<IndexerStateID>,
        limit: usize,
        descending_order: bool,
        state_type: ObjectStateType,
    ) -> IndexerResult<Vec<(ObjectID, IndexerStateID)>> {
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .indexer_reader_query_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let stored_object_state_infos = self.query_stored_object_state_infos_with_filter(
            filter,
            cursor,
            limit,
            descending_order,
            state_type,
        )?;
        let result = stored_object_state_infos
            .into_iter()
            .map(|v| v.try_parse_id())
            .collect::<Result<Vec<_>>>()
            .map_err(|e| {
                IndexerError::SQLiteReadError(format!("Cast indexer object ids failed: {:?}", e))
            })?;

        Ok(result)
    }

    pub fn query_last_indexer_state_id(
        &self,
        state_type: ObjectStateType,
    ) -> IndexerResult<Option<(i64, i64)>> {
        let table_name = get_table_name_by_state_type(state_type);
        let order_clause = format!("{TX_ORDER_STR} DESC, {STATE_INDEX_STR} DESC");
        let query = format!(
            "
                SELECT {TX_ORDER_STR},{STATE_INDEX_STR} FROM {} \
                ORDER BY {} \
                LIMIT 1
            ",
            table_name, order_clause,
        );

        tracing::debug!("query last indexer state id: {}", query);
        let stored_state_ids = self
            .get_inner_indexer_reader(table_name)?
            .run_query_with_timeout(|conn| diesel::sql_query(query).load::<StoredStateID>(conn))?;

        let last_state_id = if stored_state_ids.is_empty() {
            None
        } else {
            Some((
                stored_state_ids[0].tx_order,
                stored_state_ids[0].state_index,
            ))
        };
        Ok(last_state_id)
    }

    pub fn query_last_state_index_by_tx_order(
        &self,
        tx_order: u64,
        state_type: ObjectStateType,
    ) -> IndexerResult<Option<u64>> {
        let table_name = get_table_name_by_state_type(state_type);
        let where_clause = format!("{TX_ORDER_STR} = \"{}\"", tx_order as i64);
        let order_clause = format!("{TX_ORDER_STR} DESC, {STATE_INDEX_STR} DESC");
        let query = format!(
            "
                SELECT {TX_ORDER_STR},{STATE_INDEX_STR} FROM {} \
                WHERE {} \
                ORDER BY {} \
                LIMIT 1
            ",
            table_name, where_clause, order_clause,
        );

        tracing::debug!("query last state index by tx order: {}", query);
        let stored_state_ids = self
            .get_inner_indexer_reader(table_name)?
            .run_query_with_timeout(|conn| diesel::sql_query(query).load::<StoredStateID>(conn))?;
        let last_state_index = if stored_state_ids.is_empty() {
            None
        } else {
            Some(stored_state_ids[0].state_index as u64)
        };
        Ok(last_state_index)
    }

    #[named]
    pub fn query_fields_with_filter(
        &self,
        filter: FieldFilter,
        page: u64,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IndexerField>> {
        let start = Instant::now();
        let fn_name = function_name!();
        let _timer = self
            .metrics
            .indexer_reader_query_latency_seconds
            .with_label_values(&[fn_name])
            .start_timer();
        let page_of = page.max(1);

        let main_where_clause = match filter {
            FieldFilter::ObjectId(object_ids) => {
                let object_ids_str = object_ids
                    .into_iter()
                    .map(|obj_id| format!("\"{}\"", obj_id))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{PARENT_OBJECT_ID_STR} IN ({object_ids_str})")
            }
        };

        let order_clause = if descending_order {
            format!("{PARENT_OBJECT_ID_STR} DESC, {SORT_KEY_STR} DESC")
        } else {
            format!("{PARENT_OBJECT_ID_STR} ASC, {SORT_KEY_STR} ASC")
        };
        let mut start_limit = (page_of - 1) * (limit as u64);
        start_limit = start_limit.saturating_sub(1);
        let page_clause = format!("{}, {}", start_limit, limit);
        let query = format!(
            "
                SELECT * FROM fields \
                WHERE {} \
                ORDER BY {} \
                LIMIT {}
            ",
            main_where_clause, order_clause, page_clause,
        );

        tracing::debug!("Query fields: {}", query);
        let stored_fields = self
            .get_inner_indexer_reader(INDEXER_FIELDS_TABLE_NAME)?
            .run_query_with_timeout(|conn| diesel::sql_query(query).load::<StoredField>(conn))?;

        let result = stored_fields
            .into_iter()
            .map(|ev| ev.try_into_indexer_field())
            .collect::<Result<Vec<_>>>()
            .map_err(|e| {
                IndexerError::SQLiteReadError(format!("Cast indexer fields failed: {:?}", e))
            })?;
        tracing::debug!("Query fields time elapsed: {:?}", start.elapsed());

        Ok(result)
    }
}

fn get_table_name_by_state_type(state_type: ObjectStateType) -> IndexerTableName {
    match state_type {
        ObjectStateType::ObjectState => INDEXER_OBJECT_STATES_TABLE_NAME,
        ObjectStateType::UTXO => INDEXER_OBJECT_STATE_UTXOS_TABLE_NAME,
        ObjectStateType::Inscription => INDEXER_OBJECT_STATE_INSCRIPTIONS_TABLE_NAME,
    }
}
fn object_type_query(object_type: &StructTag) -> String {
    let object_type_str = object_type.to_string();
    // if the caller does not specify the type parameters, we will use the prefix match
    if object_type.type_params.is_empty() {
        let (first_bound, second_bound, upper_bound) =
            optimize_object_type_like_query(object_type_str.as_str());
        format!(
            "({STATE_OBJECT_TYPE_STR} = \"{}\" OR ({STATE_OBJECT_TYPE_STR} >= \"{}\" AND {STATE_OBJECT_TYPE_STR} < \"{}\" AND {STATE_OBJECT_TYPE_STR} < \"{}\"))",
            object_type_str, first_bound, second_bound, upper_bound
        )
    } else {
        format!("{STATE_OBJECT_TYPE_STR} = \"{}\"", object_type_str)
    }
}

fn not_object_type_query(object_type: &StructTag) -> String {
    let object_type_str = object_type.to_string();
    // if the caller does not specify the type parameters, we will use the prefix match
    if object_type.type_params.is_empty() {
        let (first_bound, second_bound, upper_bound) =
            optimize_object_type_like_query(object_type_str.as_str());
        format!(
            "({STATE_OBJECT_TYPE_STR} != \"{}\" AND ({STATE_OBJECT_TYPE_STR} < \"{}\" OR {STATE_OBJECT_TYPE_STR} >= \"{}\" OR {STATE_OBJECT_TYPE_STR} >= \"{}\"))",
            object_type_str, first_bound, second_bound, upper_bound
        )
    } else {
        format!("{STATE_OBJECT_TYPE_STR} != \"{}\"", object_type_str)
    }
}

// Only take effect on the rightmost prefix,
// and only include nest object type and object type itself
fn optimize_object_type_like_query(query: &str) -> (String, String, String) {
    // Nest struct start with ASCII `<`
    let first_bound = format!("{}{}", query, "<");
    // The ASCII `=` follows after the ASCII `<`
    let second_bound = format!("{}{}", query, "=");
    // Calculate the upper bound for BETWEEN AND or OR
    let upper_bound = increment_query_string(query);
    // Avoid potential SQL injection risks
    (
        escape_sql_string(first_bound),
        escape_sql_string(second_bound),
        escape_sql_string(upper_bound),
    )
}

pub fn increment_query_string(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    for i in (0..chars.len()).rev() {
        if chars[i] < char::MAX {
            chars[i] = char::from_u32(chars[i] as u32 + 1).unwrap_or(char::MAX);
            break;
        }
        chars[i] = char::from_u32(0).unwrap_or('\0');
    }

    chars.into_iter().collect()
}

#[cfg(test)]
mod test {
    use crate::indexer_reader::optimize_object_type_like_query;
    fn object_type_query_result(
        origin_object_type: String,
        match_object_type: String,
        first_bound: String,
        second_bound: String,
        upper_bound: String,
    ) -> bool {
        origin_object_type == match_object_type
            || (match_object_type >= first_bound
                && match_object_type < second_bound
                && match_object_type < upper_bound)
    }

    fn not_object_type_query_result(
        origin_object_type: String,
        match_object_type: String,
        first_bound: String,
        second_bound: String,
        upper_bound: String,
    ) -> bool {
        origin_object_type != match_object_type
            && (match_object_type < first_bound
                || match_object_type >= second_bound
                || match_object_type >= upper_bound)
    }

    #[test]
    fn test_optimize_object_type_like_query() {
        let gas_coin_object_type = "0x3::coin_store::CoinStore";
        let (first_bound, second_bound, upper_bound) =
            optimize_object_type_like_query(gas_coin_object_type);
        assert_eq!(first_bound, "0x3::coin_store::CoinStore<");
        assert_eq!(second_bound, "0x3::coin_store::CoinStore=");
        assert_eq!(upper_bound, "0x3::coin_store::CoinStorf");

        let object_type2 =
            "0x5350415253455f4d45524b4c455f504c414345484f4c4445525f484153480000::custom::CustomZZZ";
        let (first_bound, second_bound, upper_bound) =
            optimize_object_type_like_query(object_type2);
        assert_eq!(
            first_bound,
            "0x5350415253455f4d45524b4c455f504c414345484f4c4445525f484153480000::custom::CustomZZZ<"
        );
        assert_eq!(
            second_bound,
            "0x5350415253455f4d45524b4c455f504c414345484f4c4445525f484153480000::custom::CustomZZZ="
        );
        assert_eq!(
            upper_bound,
            "0x5350415253455f4d45524b4c455f504c414345484f4c4445525f484153480000::custom::CustomZZ["
        );
    }

    #[test]
    fn test_object_type_query() {
        // assert object_type_query
        let object_type = "0xabcd::test::Account";
        let object_type_include = "0xabcd::test::Account<T>".to_string();
        let object_type_exclude1 = "0xabcd::test::AccountABC".to_string();
        let object_type_exclude2 = "0xabcd::test::Account123<T>".to_string();
        let (first_bound, second_bound, upper_bound) = optimize_object_type_like_query(object_type);
        assert!(object_type_query_result(
            object_type.to_string(),
            object_type_include,
            first_bound.clone(),
            second_bound.clone(),
            upper_bound.clone()
        ));
        assert!(!object_type_query_result(
            object_type.to_string(),
            object_type_exclude1,
            first_bound.clone(),
            second_bound.clone(),
            upper_bound.clone()
        ));
        assert!(!object_type_query_result(
            object_type.to_string(),
            object_type_exclude2,
            first_bound,
            second_bound,
            upper_bound
        ));
    }

    #[test]
    fn test_not_object_type_query() {
        // assert not_object_type_query
        let object_type = "0xabcd::test::Account";
        let object_type_exclude1 = "0xabcd::test::Account".to_string();
        let object_type_exclude2 = "0xabcd::test::Account<T>".to_string();
        let object_type_include1 = "0xabcd::test::AccountABC".to_string();
        let object_type_include2 = "0xabcd::test::Account123<T>".to_string();
        let (first_bound, second_bound, upper_bound) = optimize_object_type_like_query(object_type);
        assert!(not_object_type_query_result(
            object_type.to_string(),
            object_type_include1,
            first_bound.clone(),
            second_bound.clone(),
            upper_bound.clone()
        ));
        assert!(not_object_type_query_result(
            object_type.to_string(),
            object_type_include2,
            first_bound.clone(),
            second_bound.clone(),
            upper_bound.clone()
        ));
        assert!(!not_object_type_query_result(
            object_type.to_string(),
            object_type_exclude1,
            first_bound.clone(),
            second_bound.clone(),
            upper_bound.clone()
        ));
        assert!(!not_object_type_query_result(
            object_type.to_string(),
            object_type_exclude2,
            first_bound,
            second_bound,
            upper_bound
        ));
    }
}
