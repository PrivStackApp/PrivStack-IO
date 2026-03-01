//! SQLite-backed tabular dataset storage for PrivStack.
//!
//! This crate provides a database (`data.datasets.db`) that stores raw
//! columnar data without encryption, enabling full SQL (WHERE, GROUP BY,
//! JOIN, aggregations) via SQLite.
//!
//! # Architecture
//!
//! Unlike the entity system (which encrypts all data), datasets are stored
//! as plain SQLite tables for maximum query performance. Each imported CSV
//! becomes a native SQLite table named `ds_<uuid>`.

mod error;
mod schema;
mod store;
mod types;

pub use error::{DatasetError, DatasetResult};
pub use schema::{dataset_table_name, initialize_datasets_schema};
pub use store::DatasetStore;
pub use types::{
    ColumnDef, DatasetColumn, DatasetColumnType, DatasetId, DatasetMeta, DatasetQueryResult,
    DatasetRelation, DatasetView, FilterOperator, MutationResult, PreprocessedSql, RelationType,
    RowPageLink, SavedQuery, SortDirection, SqlExecutionResult, StatementType, ViewConfig,
    ViewFilter, ViewSort,
};
