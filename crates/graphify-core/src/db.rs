use rusqlite::Connection;
use crate::error::Result;

const _CURRENT_SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS extraction_cache (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    language TEXT NOT NULL,
    nodes TEXT NOT NULL,
    edges TEXT NOT NULL,
    extracted_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS file_manifest (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    file_type TEXT NOT NULL,
    language TEXT,
    last_seen_at TEXT NOT NULL,
    size_bytes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    file_type TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_line INTEGER,
    docstring TEXT,
    community INTEGER,
    degree_centrality REAL
);
CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(source_file);
CREATE INDEX IF NOT EXISTS idx_nodes_community ON nodes(community);

CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL REFERENCES nodes(id),
    target TEXT NOT NULL REFERENCES nodes(id),
    relation TEXT NOT NULL,
    confidence TEXT NOT NULL,
    confidence_score REAL,
    source_file TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);

CREATE TABLE IF NOT EXISTS pipeline_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,
    files_processed INTEGER,
    nodes_added INTEGER,
    edges_added INTEGER
);

CREATE TABLE IF NOT EXISTS query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question TEXT NOT NULL,
    answer TEXT,
    path_taken TEXT,
    queried_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS _meta (key TEXT PRIMARY KEY, value TEXT);
INSERT OR IGNORE INTO _meta (key, value) VALUES ('schema_version', '1');
";

/// Run any pending schema migrations. Currently only v1 exists.
fn run_migrations(conn: &Connection) -> Result<()> {
    let version: i64 = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM _meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
    }
    // Future migrations go here:
    // if version < 2 { conn.execute_batch(SCHEMA_V2)?; }

    Ok(())
}

pub fn open_db(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
    let is_new = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |row| row.get::<_, i64>(0))
        .unwrap_or(0) == 0;
    if is_new {
        conn.execute_batch(SCHEMA_V1)?;
    } else {
        run_migrations(&conn)?;
    }
    Ok(conn)
}

pub fn open_db_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
    conn.execute_batch(SCHEMA_V1)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_creates_tables() {
        let conn = open_db_in_memory().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"nodes".to_string()));
        assert!(tables.contains(&"edges".to_string()));
        assert!(tables.contains(&"extraction_cache".to_string()));
        assert!(tables.contains(&"file_manifest".to_string()));
        assert!(tables.contains(&"pipeline_runs".to_string()));
        assert!(tables.contains(&"query_history".to_string()));
    }

    #[test]
    fn indexes_exist() {
        let conn = open_db_in_memory().unwrap();
        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(indexes.contains(&"idx_nodes_file".to_string()));
        assert!(indexes.contains(&"idx_nodes_community".to_string()));
        assert!(indexes.contains(&"idx_edges_source".to_string()));
        assert!(indexes.contains(&"idx_edges_target".to_string()));
    }

    #[test]
    fn insert_and_query_node() {
        let conn = open_db_in_memory().unwrap();
        conn.execute(
            "INSERT INTO nodes (id, label, file_type, source_file, source_line) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["main.py::Foo", "Foo", "code", "main.py", 10],
        ).unwrap();

        let label: String = conn
            .query_row("SELECT label FROM nodes WHERE id = ?1", rusqlite::params!["main.py::Foo"], |row| row.get(0))
            .unwrap();
        assert_eq!(label, "Foo");
    }
}
