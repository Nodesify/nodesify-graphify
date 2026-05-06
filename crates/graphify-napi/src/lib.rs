pub mod export_graphml;
pub mod export_html;
pub mod merge;
pub mod pipeline;
pub mod query;

use napi_derive::napi;
use std::path::PathBuf;

// ---- napi-exposed types ----

#[napi(object)]
pub struct PipelineResultJs {
    pub nodes_added: i64,
    pub edges_added: i64,
    pub communities: i64,
    pub report: String,
}

#[napi(object)]
pub struct GraphStatsJs {
    pub node_count: i64,
    pub edge_count: i64,
    pub community_count: i64,
    pub file_count: i64,
}

#[napi(object)]
pub struct QueryResultJs {
    pub text: String,
    pub node_count: i64,
    pub edge_count: i64,
}

#[napi(object)]
pub struct PathResultJs {
    pub found: bool,
    pub hops: i64,
    pub text: String,
}

#[napi(object)]
pub struct EdgeInfoJs {
    pub neighbor_id: String,
    pub neighbor_label: String,
    pub neighbor_file: String,
    pub relation: String,
    pub confidence: String,
}

#[napi(object)]
pub struct ExplainResultJs {
    pub id: String,
    pub label: String,
    pub source_file: String,
    pub community: Option<i64>,
    pub neighbor_count: i64,
    pub neighbors: Vec<EdgeInfoJs>,
}

#[napi(object)]
pub struct DiffResultJs {
    pub nodes_added: i64,
    pub nodes_removed: i64,
    pub edges_added: i64,
    pub edges_removed: i64,
    pub added_node_labels: Vec<String>,
    pub removed_node_labels: Vec<String>,
}

#[napi(object)]
pub struct HistoryEntryJs {
    pub id: i64,
    pub question: String,
    pub answer: Option<String>,
    pub queried_at: String,
}

// ---- napi-exposed functions ----

#[napi]
pub fn run_pipeline(root: String) -> napi::Result<PipelineResultJs> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(
        &graphify_paths::db_path(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?,
    );
    let result =
        pipeline::run_pipeline(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    query::invalidate_graph_cache(&db_path_str);
    Ok(PipelineResultJs {
        nodes_added: result.build_result.nodes_added as i64,
        edges_added: result.build_result.edges_added as i64,
        communities: result.cluster_result.communities.len() as i64,
        report: result.report,
    })
}

/// Incremental rebuild — intentionally reuses run_pipeline because the pipeline
/// internally detects changed files via SHA-256 manifest and skips unchanged ones.
#[napi]
pub fn update_pipeline(root: String) -> napi::Result<PipelineResultJs> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(
        &graphify_paths::db_path(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?,
    );
    let result =
        pipeline::run_pipeline(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    query::invalidate_graph_cache(&db_path_str);
    Ok(PipelineResultJs {
        nodes_added: result.build_result.nodes_added as i64,
        edges_added: result.build_result.edges_added as i64,
        communities: result.cluster_result.communities.len() as i64,
        report: result.report,
    })
}

#[napi]
pub fn graph_stats(root: String) -> napi::Result<GraphStatsJs> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let node_count: i64 = db
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap_or(0);
    let edge_count: i64 = db
        .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
        .unwrap_or(0);
    let community_count: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT community) FROM nodes WHERE community IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let file_count: i64 = db
        .query_row("SELECT COUNT(*) FROM file_manifest", [], |r| r.get(0))
        .unwrap_or(0);
    Ok(GraphStatsJs {
        node_count,
        edge_count,
        community_count,
        file_count,
    })
}

#[napi]
pub fn export_json_cmd(root: String, out_path: String) -> napi::Result<()> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    pipeline::export_json(&db, &PathBuf::from(&out_path))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(())
}

#[napi]
pub fn export_html_cmd(root: String, out_path: String) -> napi::Result<()> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    export_html::export_html(&db, &PathBuf::from(&out_path))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(())
}

#[napi]
pub fn export_graphml_cmd(root: String, out_path: String) -> napi::Result<()> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    export_graphml::export_graphml(&db, &PathBuf::from(&out_path))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(())
}

#[napi]
pub fn query_graph(
    root: String,
    question: String,
    mode: String,
    depth: i64,
    budget: i64,
) -> napi::Result<QueryResultJs> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(
        &graphify_paths::db_path(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?,
    );
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let (text, node_count, edge_count) =
        query::query_graph(&db, &db_path_str, &question, &mode, depth as usize, budget)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(QueryResultJs {
        text,
        node_count: node_count as i64,
        edge_count: edge_count as i64,
    })
}

#[napi]
pub fn find_path(root: String, source: String, target: String) -> napi::Result<PathResultJs> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(
        &graphify_paths::db_path(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?,
    );
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let (found, hops, text) = query::find_shortest_path(&db, &db_path_str, &source, &target)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(PathResultJs {
        found,
        hops: hops as i64,
        text,
    })
}

#[napi]
pub fn explain_node(root: String, node_id: String) -> napi::Result<Option<ExplainResultJs>> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(
        &graphify_paths::db_path(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?,
    );
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let result = query::explain_with_neighbors(&db, &db_path_str, &node_id)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(result.map(|r| ExplainResultJs {
        id: r.id,
        label: r.label,
        source_file: r.source_file,
        community: r.community,
        neighbor_count: r.neighbor_count as i64,
        neighbors: r
            .neighbors
            .into_iter()
            .map(|n| EdgeInfoJs {
                neighbor_id: n.neighbor_id,
                neighbor_label: n.neighbor_label,
                neighbor_file: n.neighbor_file,
                relation: n.relation,
                confidence: n.confidence,
            })
            .collect(),
    }))
}

#[napi]
pub fn cluster_only(root: String) -> napi::Result<PipelineResultJs> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(
        &graphify_paths::db_path(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?,
    );
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let cluster_result =
        graphify_cluster::cluster(&db).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let analysis =
        graphify_analyze::analyze(&db).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let report = graphify_report::generate_report(&db, &analysis)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let graphify_dir = root_pb.join(".graphify");
    let _ = std::fs::write(graphify_dir.join("graph_report.md"), &report);

    query::invalidate_graph_cache(&db_path_str);

    Ok(PipelineResultJs {
        nodes_added: 0,
        edges_added: 0,
        communities: cluster_result.communities.len() as i64,
        report,
    })
}

#[napi]
pub fn merge_graphs(
    root_a: String,
    root_b: String,
    out_root: String,
) -> napi::Result<PipelineResultJs> {
    let result = merge::merge_graphs(
        &PathBuf::from(&root_a),
        &PathBuf::from(&root_b),
        &PathBuf::from(&out_root),
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(PipelineResultJs {
        nodes_added: result.nodes_added,
        edges_added: result.edges_added,
        communities: result.communities as i64,
        report: result.report,
    })
}

#[napi]
pub fn diff_graphs(root_a: String, root_b: String) -> napi::Result<DiffResultJs> {
    let result = merge::diff_graphs(&PathBuf::from(&root_a), &PathBuf::from(&root_b))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(DiffResultJs {
        nodes_added: result.nodes_added,
        nodes_removed: result.nodes_removed,
        edges_added: result.edges_added,
        edges_removed: result.edges_removed,
        added_node_labels: result.added_node_labels,
        removed_node_labels: result.removed_node_labels,
    })
}

#[napi]
pub fn graph_history(root: String, limit: i64) -> napi::Result<Vec<HistoryEntryJs>> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let mut stmt = db
        .prepare(
            "SELECT id, question, answer, queried_at FROM query_history ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let entries: Vec<HistoryEntryJs> = stmt
        .query_map(rusqlite::params![limit], |row| {
            Ok(HistoryEntryJs {
                id: row.get(0)?,
                question: row.get(1)?,
                answer: row.get(2)?,
                queried_at: row.get(3)?,
            })
        })
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use crate::pipeline;
    use crate::query;
    use graphify_core::db::open_db_in_memory;

    fn seed_graph(
        db: &rusqlite::Connection,
        nodes: &[(&str, &str, &str, Option<i64>)],
        edges: &[(&str, &str, &str)],
    ) {
        for &(id, label, sf, community) in nodes {
            db.execute(
                "INSERT INTO nodes (id, label, file_type, source_file, community) VALUES (?1, ?2, 'code', ?3, ?4)",
                rusqlite::params![id, label, sf, community],
            ).unwrap();
        }
        for &(src, tgt, rel) in edges {
            db.execute(
                "INSERT INTO edges (source, target, relation, confidence, source_file) VALUES (?1, ?2, ?3, 'EXTRACTED', 'test.py')",
                rusqlite::params![src, tgt, rel],
            ).unwrap();
        }
    }

    #[test]
    fn query_graph_empty_db_returns_no_nodes() {
        let db = open_db_in_memory().unwrap();
        let key = format!(":memory:empty_{}", std::process::id());
        let (text, nodes, edges) =
            query::query_graph(&db, &key, "anything", "bfs", 3, 2000).unwrap();
        assert_eq!(text, "No nodes in graph.");
        assert_eq!(nodes, 0);
        assert_eq!(edges, 0);
    }

    #[test]
    fn query_graph_no_matching_nodes() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db, &[("n1", "Alpha", "f.py", None)], &[]);
        let key = format!(":memory:nomatch_{}", std::process::id());
        let (text, nodes, _) =
            query::query_graph(&db, &key, "xyznonexistent", "bfs", 3, 2000).unwrap();
        assert_eq!(text, "No matching nodes found.");
        assert_eq!(nodes, 0);
    }

    #[test]
    fn query_graph_bfs_finds_subgraph() {
        let db = open_db_in_memory().unwrap();
        seed_graph(
            &db,
            &[
                ("n1", "Alpha", "f.py", Some(0)),
                ("n2", "Beta", "f.py", Some(0)),
                ("n3", "Gamma", "g.py", Some(1)),
            ],
            &[("n1", "n2", "calls"), ("n2", "n3", "imports")],
        );
        let key = format!(":memory:bfs_{}", std::process::id());
        let (text, nodes, _edges) = query::query_graph(&db, &key, "Alpha", "bfs", 2, 2000).unwrap();
        assert!(nodes > 0);
        assert!(text.contains("Alpha"));
    }

    #[test]
    fn query_graph_dfs_finds_subgraph() {
        let db = open_db_in_memory().unwrap();
        seed_graph(
            &db,
            &[("n1", "Alpha", "f.py", None), ("n2", "Beta", "f.py", None)],
            &[("n1", "n2", "calls")],
        );
        let key = format!(":memory:dfs_{}", std::process::id());
        let (text, nodes, _) = query::query_graph(&db, &key, "Alpha", "dfs", 2, 2000).unwrap();
        assert!(nodes > 0);
        assert!(text.contains("Alpha"));
    }

    #[test]
    fn find_shortest_path_found() {
        let db = open_db_in_memory().unwrap();
        seed_graph(
            &db,
            &[
                ("n1", "Alpha", "f.py", None),
                ("n2", "Beta", "f.py", None),
                ("n3", "Gamma", "g.py", None),
            ],
            &[("n1", "n2", "calls"), ("n2", "n3", "calls")],
        );
        let key = format!(":memory:path_{}", std::process::id());
        let (found, hops, text) = query::find_shortest_path(&db, &key, "Alpha", "Gamma").unwrap();
        assert!(found);
        assert_eq!(hops, 2);
        assert!(text.contains("Alpha"));
        assert!(text.contains("Gamma"));
    }

    #[test]
    fn find_shortest_path_no_path() {
        let db = open_db_in_memory().unwrap();
        seed_graph(
            &db,
            &[("n1", "Alpha", "f.py", None), ("n2", "Beta", "f.py", None)],
            &[],
        );
        let key = format!(":memory:nopath_{}", std::process::id());
        let (found, hops, _) = query::find_shortest_path(&db, &key, "Alpha", "Beta").unwrap();
        assert!(!found);
        assert_eq!(hops, 0);
    }

    #[test]
    fn find_shortest_path_no_match() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db, &[("n1", "Alpha", "f.py", None)], &[]);
        let key = format!(":memory:nomatchpath_{}", std::process::id());
        let (found, _, text) =
            query::find_shortest_path(&db, &key, "Alpha", "Nonexistent").unwrap();
        assert!(!found);
        assert!(text.contains("No matching node"));
    }

    #[test]
    fn find_shortest_path_same_node() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db, &[("n1", "Alpha", "f.py", None)], &[]);
        let key = format!(":memory:same_{}", std::process::id());
        let (found, hops, _) = query::find_shortest_path(&db, &key, "Alpha", "Alpha").unwrap();
        assert!(found);
        assert_eq!(hops, 0);
    }

    #[test]
    fn explain_node_found() {
        let db = open_db_in_memory().unwrap();
        seed_graph(
            &db,
            &[
                ("n1", "Alpha", "f.py", Some(0)),
                ("n2", "Beta", "f.py", Some(0)),
            ],
            &[("n1", "n2", "calls")],
        );
        let key = format!(":memory:explain_{}", std::process::id());
        let result = query::explain_with_neighbors(&db, &key, "n1").unwrap();
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.label, "Alpha");
        assert_eq!(r.neighbor_count, 1);
        assert_eq!(r.neighbors[0].neighbor_label, "Beta");
    }

    #[test]
    fn explain_node_not_found() {
        let db = open_db_in_memory().unwrap();
        let key = format!(":memory:explainnf_{}", std::process::id());
        let result = query::explain_with_neighbors(&db, &key, "nonexistent_xyz").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn export_json_writes_valid_file() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db, &[("n1", "Alpha", "f.py", Some(0))], &[]);
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("graph.json");
        pipeline::export_json(&db, &out).unwrap();
        let json_str = std::fs::read_to_string(&out).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed["nodes"].as_array().unwrap().len() == 1);
        assert!(parsed["edges"].as_array().unwrap().is_empty());
    }
}
