// graphify-core: core types, database schema, and pipeline orchestration

pub mod db;
pub mod error;
pub mod ids;
pub mod security;
pub mod types;

/// Version tag mixed into every content hash (detect manifest + extraction
/// cache). Bump when extraction output changes shape (e.g. the id scheme) —
/// all files then hash differently, forcing one clean full re-extraction on
/// upgrade instead of mixing old and new node ids in one graph.
pub const EXTRACTION_HASH_VERSION: &str = "v3";

pub use db::{open_db, open_db_in_memory};
pub use error::{GraphifyError, Result};
pub use security::{check_file_size, sanitize_docstring, sanitize_label, validate_path};
pub use types::*;
