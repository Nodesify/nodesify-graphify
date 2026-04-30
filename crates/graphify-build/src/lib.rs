// graphify-build: merge extractions into SQLite graph

use graphify_core::Result;
use graphify_extract::Extraction;
use rusqlite::{Connection, Transaction};

#[derive(Debug)]
pub struct BuildResult {
    pub nodes_added: usize,
    pub edges_added: usize,
    pub duplicates_merged: usize,
}

pub fn build(extractions: &[Extraction], db: &Connection) -> Result<BuildResult> {
    let mut nodes_added = 0;
    let mut edges_added = 0;
    let mut duplicates_merged = 0;

    let tx = db.unchecked_transaction()?;

    for extraction in extractions {
        let file_path = extraction
            .file_path
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        // Delete old edges first (foreign key references nodes), then old nodes
        tx.execute(
            "DELETE FROM edges WHERE source_file = ?1",
            rusqlite::params![file_path],
        )?;
        tx.execute(
            "DELETE FROM nodes WHERE source_file = ?1",
            rusqlite::params![file_path],
        )?;

        for node in &extraction.nodes {
            // Check if a node with this id already exists from another file
            let existing: bool = tx
                .query_row(
                    "SELECT COUNT(*) FROM nodes WHERE id = ?1",
                    rusqlite::params![node.id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0;

            if existing {
                duplicates_merged += 1;
                continue;
            }

            tx.execute(
                "INSERT OR IGNORE INTO nodes (id, label, file_type, source_file, source_line, docstring) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    node.id,
                    node.label,
                    "code",
                    normalize_path(&node.source_file),
                    node.source_line,
                    node.docstring,
                ],
            )?;
            nodes_added += 1;
        }

        for edge in &extraction.edges {
            // Ensure source node exists (stub if missing)
            ensure_node_exists(&tx, &edge.source, &edge.source, &normalize_path(&edge.source_file))?;
            // Ensure target node exists (stub if missing)
            ensure_node_exists(&tx, &edge.target, &edge.target, &normalize_path(&edge.source_file))?;

            tx.execute(
                "INSERT INTO edges (source, target, relation, confidence, confidence_score, source_file) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    edge.source,
                    edge.target,
                    edge.relation,
                    edge.confidence,
                    edge.confidence_score,
                    normalize_path(&edge.source_file),
                ],
            )?;
            edges_added += 1;
        }
    }

    tx.commit()?;
    Ok(BuildResult {
        nodes_added,
        edges_added,
        duplicates_merged,
    })
}

/// Ensure a node with the given id exists. If not, insert a stub node.
fn ensure_node_exists(tx: &Transaction, id: &str, label: &str, source_file: &str) -> Result<()> {
    let exists: bool = tx
        .query_row(
            "SELECT COUNT(*) FROM nodes WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !exists {
        tx.execute(
            "INSERT OR IGNORE INTO nodes (id, label, file_type, source_file) VALUES (?1, ?2, 'stub', ?3)",
            rusqlite::params![id, label, source_file],
        )?;
    }
    Ok(())
}

/// Normalize a path to use forward slashes for consistent storage.
fn normalize_path(path: &std::path::Path) -> String {
    path.to_string_lossy().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;
    use graphify_extract::{Extraction, ExtractedEdge, ExtractedNode};
    use std::path::PathBuf;

    fn make_extraction(nodes: Vec<(&str, &str)>, edges: Vec<(&str, &str, &str)>) -> Extraction {
        Extraction {
            file_path: PathBuf::from("test.py"),
            language: "Python".into(),
            nodes: nodes
                .into_iter()
                .map(|(id, label)| ExtractedNode {
                    id: id.into(),
                    label: label.into(),
                    source_file: PathBuf::from("test.py"),
                    source_line: Some(1),
                    docstring: None,
                    node_type: "function".into(),
                })
                .collect(),
            edges: edges
                .into_iter()
                .map(|(src, tgt, rel)| ExtractedEdge {
                    source: src.into(),
                    target: tgt.into(),
                    relation: rel.into(),
                    confidence: "EXTRACTED".into(),
                    confidence_score: Some(1.0),
                    source_file: PathBuf::from("test.py"),
                    source_line: Some(1),
                })
                .collect(),
        }
    }

    #[test]
    fn build_inserts_nodes_and_edges() {
        let db = open_db_in_memory().unwrap();
        let ext = make_extraction(
            vec![("testpy::hello", "hello()"), ("testpy::world", "world()")],
            vec![("testpy::hello", "testpy::world", "calls")],
        );
        let result = build(&[ext], &db).unwrap();
        assert_eq!(result.nodes_added, 2);
        assert_eq!(result.edges_added, 1);
    }

    #[test]
    fn build_replaces_file_on_rebuild() {
        let db = open_db_in_memory().unwrap();
        let ext1 = make_extraction(vec![("testpy::old", "old()")], vec![]);
        build(&[ext1], &db).unwrap();

        let ext2 = make_extraction(vec![("testpy::new", "new()")], vec![]);
        let result = build(&[ext2], &db).unwrap();
        assert_eq!(result.nodes_added, 1);

        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM nodes WHERE id = 'testpy::old'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn build_deduplicates_nodes_across_files() {
        let db = open_db_in_memory().unwrap();

        let ext1 = Extraction {
            file_path: PathBuf::from("a.py"),
            language: "Python".into(),
            nodes: vec![ExtractedNode {
                id: "shared::func".into(),
                label: "func()".into(),
                source_file: PathBuf::from("a.py"),
                source_line: Some(1),
                docstring: None,
                node_type: "function".into(),
            }],
            edges: vec![],
        };
        let result1 = build(&[ext1], &db).unwrap();
        assert_eq!(result1.nodes_added, 1);
        assert_eq!(result1.duplicates_merged, 0);

        let ext2 = Extraction {
            file_path: PathBuf::from("b.py"),
            language: "Python".into(),
            nodes: vec![ExtractedNode {
                id: "shared::func".into(),
                label: "func()".into(),
                source_file: PathBuf::from("b.py"),
                source_line: Some(5),
                docstring: None,
                node_type: "function".into(),
            }],
            edges: vec![],
        };
        let result2 = build(&[ext2], &db).unwrap();
        assert_eq!(result2.nodes_added, 0);
        assert_eq!(result2.duplicates_merged, 1);
    }

    #[test]
    fn build_handles_empty_extractions() {
        let db = open_db_in_memory().unwrap();
        let ext = Extraction {
            file_path: PathBuf::from("empty.py"),
            language: "Python".into(),
            nodes: vec![],
            edges: vec![],
        };
        let result = build(&[ext], &db).unwrap();
        assert_eq!(result.nodes_added, 0);
        assert_eq!(result.edges_added, 0);
        assert_eq!(result.duplicates_merged, 0);
    }

    #[test]
    fn build_multiple_extractions_in_one_call() {
        let db = open_db_in_memory().unwrap();

        let ext1 = Extraction {
            file_path: PathBuf::from("a.py"),
            language: "Python".into(),
            nodes: vec![ExtractedNode {
                id: "a::foo".into(),
                label: "foo()".into(),
                source_file: PathBuf::from("a.py"),
                source_line: Some(1),
                docstring: None,
                node_type: "function".into(),
            }],
            edges: vec![],
        };

        let ext2 = Extraction {
            file_path: PathBuf::from("b.py"),
            language: "Python".into(),
            nodes: vec![ExtractedNode {
                id: "b::bar".into(),
                label: "bar()".into(),
                source_file: PathBuf::from("b.py"),
                source_line: Some(1),
                docstring: None,
                node_type: "function".into(),
            }],
            edges: vec![ExtractedEdge {
                source: "b::bar".into(),
                target: "a::foo".into(),
                relation: "calls".into(),
                confidence: "EXTRACTED".into(),
                confidence_score: Some(1.0),
                source_file: PathBuf::from("b.py"),
                source_line: Some(2),
            }],
        };

        let result = build(&[ext1, ext2], &db).unwrap();
        assert_eq!(result.nodes_added, 2);
        assert_eq!(result.edges_added, 1);
    }
}
