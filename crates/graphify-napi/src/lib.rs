pub mod export_graphml;
pub mod export_html;
pub mod export_tree;
pub mod merge;
pub mod pipeline;
pub mod query;

use napi_derive::napi;
use std::path::{Path, PathBuf};

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
    /// Some when the node list was truncated - pass back as `cursor`.
    pub next_cursor: Option<i64>,
}

#[napi(object)]
pub struct RepoMapJs {
    pub text: String,
    pub files_shown: i64,
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

#[napi(object)]
pub struct AffectedHitJs {
    pub id: String,
    pub label: String,
    pub depth: i32,
    pub relation: String,
    pub via_file: String,
}

#[napi(object)]
pub struct AffectedResultJs {
    pub seed: String,
    pub seed_label: String,
    pub total: i32,
    pub hits: Vec<AffectedHitJs>,
}

// ---- napi-exposed functions ----

/// Fidelity tier: "high" keeps only EXTRACTED/DECLARED facts (strength
/// >= 0.9); anything else keeps all facts.
fn min_strength_for(detail: &Option<String>) -> f64 {
    match detail.as_deref().map(|s| s.to_lowercase()).as_deref() {
        Some("high") => 0.9,
        _ => 0.0,
    }
}

#[napi]
pub fn run_pipeline(root: String, no_dedup: Option<bool>) -> napi::Result<PipelineResultJs> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(&graphify_paths::db_path(
        &root_pb
            .canonicalize()
            .map_err(|e| napi::Error::from_reason(e.to_string()))?,
    )?);
    let result = pipeline::run_pipeline_with(&root_pb, !no_dedup.unwrap_or(false))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
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
pub fn update_pipeline(root: String, no_dedup: Option<bool>) -> napi::Result<PipelineResultJs> {
    let root_pb = PathBuf::from(&root);
    let db_path_str = graphify_paths::normalize(&graphify_paths::db_path(
        &root_pb
            .canonicalize()
            .map_err(|e| napi::Error::from_reason(e.to_string()))?,
    )?);
    let result = pipeline::run_pipeline_with(&root_pb, !no_dedup.unwrap_or(false))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
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
pub fn export_html_cmd(root: String, out_path: String, mode: Option<String>) -> napi::Result<()> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let mode = export_html::HtmlExportMode::parse(mode.as_deref().unwrap_or("standard"))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    export_html::export_html_with_mode(&db, &PathBuf::from(&out_path), mode)
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
#[allow(clippy::too_many_arguments)]
pub fn query_graph(
    root: String,
    question: String,
    mode: String,
    depth: i64,
    budget: i64,
    directed: Option<bool>,
    detail: Option<String>,
    cursor: Option<i64>,
) -> napi::Result<QueryResultJs> {
    let root_pb = PathBuf::from(&root);
    let root_pb = root_pb
        .canonicalize()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    // Load before touching path helpers: a missing graph must error without
    // creating an empty `.graphify/` directory as a side effect.
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db_path_str = graphify_paths::normalize(&root_pb.join(".graphify").join("db.sqlite"));
    let (text, node_count, edge_count, next_cursor) = query::query_graph(
        &db,
        &db_path_str,
        &question,
        &mode,
        depth as usize,
        budget,
        directed.unwrap_or(false),
        min_strength_for(&detail),
        cursor.unwrap_or(0).max(0) as usize,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(QueryResultJs {
        text,
        node_count: node_count as i64,
        edge_count: edge_count as i64,
        next_cursor: next_cursor.map(|c| c as i64),
    })
}

#[napi]
pub fn repo_map(root: String, budget: i64, detail: Option<String>) -> napi::Result<RepoMapJs> {
    let root_pb = PathBuf::from(&root);
    let root_pb = root_pb
        .canonicalize()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    // Load before touching path helpers: a missing graph must error without
    // creating an empty `.graphify/` directory as a side effect.
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db_path_str = graphify_paths::normalize(&root_pb.join(".graphify").join("db.sqlite"));
    let (text, files_shown) = query::repo_map(&db, &db_path_str, budget, min_strength_for(&detail))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(RepoMapJs {
        text,
        files_shown: files_shown as i64,
    })
}

#[napi]
pub fn find_path(
    root: String,
    source: String,
    target: String,
    directed: Option<bool>,
    detail: Option<String>,
) -> napi::Result<PathResultJs> {
    let root_pb = PathBuf::from(&root);
    let root_pb = root_pb
        .canonicalize()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    // Load before touching path helpers: a missing graph must error without
    // creating an empty `.graphify/` directory as a side effect.
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db_path_str = graphify_paths::normalize(&root_pb.join(".graphify").join("db.sqlite"));
    let (found, hops, text) = query::find_shortest_path(
        &db,
        &db_path_str,
        &source,
        &target,
        directed.unwrap_or(false),
        min_strength_for(&detail),
    )
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
    let root_pb = root_pb
        .canonicalize()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    // Load before touching path helpers: a missing graph must error without
    // creating an empty `.graphify/` directory as a side effect.
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db_path_str = graphify_paths::normalize(&root_pb.join(".graphify").join("db.sqlite"));
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
pub fn affected_node(
    root: String,
    node: String,
    depth: Option<u32>,
    relation: Option<String>,
) -> napi::Result<AffectedResultJs> {
    let root_pb = PathBuf::from(&root);
    // Report via_file hit paths relative to the project root.
    let root_str = root_pb
        .canonicalize()
        .map(|p| graphify_paths::normalize(&p))
        .unwrap_or(root.clone());
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let result =
        graphify_analyze::affected::affected(&db, &node, depth.unwrap_or(2), relation.as_deref())
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(AffectedResultJs {
        seed: result.seed,
        seed_label: result.seed_label,
        total: result.total as i32,
        hits: result
            .hits
            .into_iter()
            .map(|h| AffectedHitJs {
                id: h.id,
                label: h.label,
                depth: h.depth as i32,
                relation: h.relation,
                via_file: graphify_paths::relative_display(&h.via_file, &root_str),
            })
            .collect(),
    })
}

#[napi]
pub fn export_tree(root: String, out: String, max_children: Option<i32>) -> napi::Result<i32> {
    let root_pb = PathBuf::from(&root);
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let out_path = Path::new(&out);
    let out_path = if out_path.is_absolute() {
        out_path.to_path_buf()
    } else {
        root_pb.join(out_path)
    };
    let count =
        export_tree::export_tree(&db, &out_path, max_children.unwrap_or(40).max(1) as usize)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(count as i32)
}

#[napi(object)]
pub struct IngestResultJs {
    pub saved_path: String,
    pub graph_updated: bool,
}

#[napi]
pub fn ingest_url(
    root: String,
    url: String,
    author: Option<String>,
    contributor: Option<String>,
) -> napi::Result<IngestResultJs> {
    let root_pb = PathBuf::from(&root);
    if !root_pb.exists() {
        return Err(napi::Error::from_reason(format!(
            "path does not exist: {}",
            root_pb.display()
        )));
    }
    let db_path_str = graphify_paths::normalize(&graphify_paths::db_path(
        &root_pb
            .canonicalize()
            .map_err(|e| napi::Error::from_reason(e.to_string()))?,
    )?);

    let opts = graphify_ingest::IngestOptions {
        author,
        contributor,
    };
    let raw_dir = root_pb.join("raw");
    let saved = graphify_ingest::ingest_url(&url, &raw_dir, &opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    // Incremental update picks the new file up (hash manifest sees it as new)
    pipeline::run_pipeline_with(&root_pb, true)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    query::invalidate_graph_cache(&db_path_str);

    Ok(IngestResultJs {
        saved_path: graphify_paths::normalize(&saved),
        graph_updated: true,
    })
}

#[napi]
pub fn run_mcp_server(root: String) -> napi::Result<()> {
    let root_pb = PathBuf::from(&root);
    if !root_pb.exists() {
        return Err(napi::Error::from_reason(format!(
            "path does not exist: {}",
            root_pb.display()
        )));
    }
    let root_pb = root_pb
        .canonicalize()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    // Refuse to serve (or create) a graph that was never built: the MCP
    // server is read-only, so a missing graph is a hard error — otherwise
    // agents would connect to an empty graph with no hint why.
    let db_path = root_pb.join(".graphify").join("db.sqlite");
    if !db_path.exists() {
        return Err(napi::Error::from_reason(format!(
            "No graph found at {} — run `nodesify-graphify run <path>` first",
            db_path.display()
        )));
    }
    graphify_mcp::serve(&db_path).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn cluster_only(root: String) -> napi::Result<PipelineResultJs> {
    let root_pb = PathBuf::from(&root);
    let root_pb = root_pb
        .canonicalize()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db =
        pipeline::load_graph_db(&root_pb).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db_path_str = graphify_paths::normalize(&root_pb.join(".graphify").join("db.sqlite"));

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
        let (text, nodes, edges, _) =
            query::query_graph(&db, &key, "anything", "bfs", 3, 2000, false, 0.0, 0).unwrap();
        assert_eq!(text, "No nodes in graph.");
        assert_eq!(nodes, 0);
        assert_eq!(edges, 0);
    }

    #[test]
    fn query_graph_no_matching_nodes() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db, &[("n1", "Alpha", "f.py", None)], &[]);
        let key = format!(":memory:nomatch_{}", std::process::id());
        let (text, nodes, _, _) =
            query::query_graph(&db, &key, "xyznonexistent", "bfs", 3, 2000, false, 0.0, 0).unwrap();
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
        let (text, nodes, _edges, _) =
            query::query_graph(&db, &key, "Alpha", "bfs", 2, 2000, false, 0.0, 0).unwrap();
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
        let (text, nodes, _, _) =
            query::query_graph(&db, &key, "Alpha", "dfs", 2, 2000, false, 0.0, 0).unwrap();
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
        let (found, hops, text) =
            query::find_shortest_path(&db, &key, "Alpha", "Gamma", false, 0.0).unwrap();
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
        let (found, hops, _) =
            query::find_shortest_path(&db, &key, "Alpha", "Beta", false, 0.0).unwrap();
        assert!(!found);
        assert_eq!(hops, 0);
    }

    #[test]
    fn find_shortest_path_no_match() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db, &[("n1", "Alpha", "f.py", None)], &[]);
        let key = format!(":memory:nomatchpath_{}", std::process::id());
        let (found, _, text) =
            query::find_shortest_path(&db, &key, "Alpha", "Nonexistent", false, 0.0).unwrap();
        assert!(!found);
        assert!(text.contains("No matching node"));
    }

    #[test]
    fn find_shortest_path_same_node() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db, &[("n1", "Alpha", "f.py", None)], &[]);
        let key = format!(":memory:same_{}", std::process::id());
        let (found, hops, _) =
            query::find_shortest_path(&db, &key, "Alpha", "Alpha", false, 0.0).unwrap();
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
