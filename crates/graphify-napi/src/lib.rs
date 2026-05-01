pub mod export_html;
pub mod export_graphml;
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
pub struct NodeJs {
    pub id: String,
    pub label: String,
    pub file_type: String,
    pub source_file: String,
    pub source_line: Option<i64>,
    pub docstring: Option<String>,
    pub community: Option<i64>,
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
    let result = pipeline::run_pipeline(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
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
    let result = pipeline::run_pipeline(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
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
    let node_count: i64 = db.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0)).unwrap_or(0);
    let edge_count: i64 = db.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0)).unwrap_or(0);
    let community_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT community) FROM nodes WHERE community IS NOT NULL", [], |r| r.get(0)
    ).unwrap_or(0);
    let file_count: i64 = db.query_row("SELECT COUNT(*) FROM file_manifest", [], |r| r.get(0)).unwrap_or(0);
    Ok(GraphStatsJs { node_count, edge_count, community_count, file_count })
}

#[napi]
pub fn get_node(root: String, node_id: String) -> napi::Result<Option<NodeJs>> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let result = db.query_row(
        "SELECT id, label, file_type, source_file, source_line, docstring, community FROM nodes WHERE id = ?1",
        rusqlite::params![node_id],
        |row| Ok(NodeJs {
            id: row.get(0)?,
            label: row.get(1)?,
            file_type: row.get(2)?,
            source_file: row.get(3)?,
            source_line: row.get::<_, Option<i64>>(4)?,
            docstring: row.get(5)?,
            community: row.get::<_, Option<i64>>(6)?,
        }),
    );
    match result {
        Ok(node) => Ok(Some(node)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(napi::Error::from_reason(e.to_string())),
    }
}

#[napi]
pub fn get_neighbors(root: String, node_id: String) -> napi::Result<Vec<NodeJs>> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let mut stmt = db.prepare(
        "SELECT DISTINCT n.id, n.label, n.file_type, n.source_file, n.source_line, n.docstring, n.community
         FROM nodes n
         JOIN edges e ON (e.target = n.id AND e.source = ?1) OR (e.source = n.id AND e.target = ?1)
         WHERE n.id != ?1"
    ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let nodes: Vec<NodeJs> = stmt.query_map(rusqlite::params![node_id], |row| {
        Ok(NodeJs {
            id: row.get(0)?, label: row.get(1)?, file_type: row.get(2)?,
            source_file: row.get(3)?, source_line: row.get::<_, Option<i64>>(4)?,
            docstring: row.get(5)?, community: row.get::<_, Option<i64>>(6)?,
        })
    }).map_err(|e| napi::Error::from_reason(e.to_string()))?
    .filter_map(|r| r.ok()).collect();
    Ok(nodes)
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
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let (text, node_count, edge_count) = query::query_graph(&db, &question, &mode, depth as usize, budget)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(QueryResultJs {
        text,
        node_count: node_count as i64,
        edge_count: edge_count as i64,
    })
}

#[napi]
pub fn find_path(root: String, source: String, target: String) -> napi::Result<PathResultJs> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let (found, hops, text) = query::find_shortest_path(&db, &source, &target)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(PathResultJs {
        found,
        hops: hops as i64,
        text,
    })
}

#[napi]
pub fn explain_node(root: String, node_id: String) -> napi::Result<Option<ExplainResultJs>> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let result = query::explain_with_neighbors(&db, &node_id)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(result.map(|r| ExplainResultJs {
        id: r.id,
        label: r.label,
        source_file: r.source_file,
        community: r.community,
        neighbor_count: r.neighbor_count as i64,
        neighbors: r.neighbors.into_iter().map(|n| EdgeInfoJs {
            neighbor_id: n.neighbor_id,
            neighbor_label: n.neighbor_label,
            neighbor_file: n.neighbor_file,
            relation: n.relation,
            confidence: n.confidence,
        }).collect(),
    }))
}

#[napi]
pub fn cluster_only(root: String) -> napi::Result<PipelineResultJs> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let cluster_result = graphify_cluster::cluster(&db)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let analysis = graphify_analyze::analyze(&db)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let report = graphify_report::generate_report(&db, &analysis)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    // Write report
    let graphify_dir = PathBuf::from(&root).join(".graphify");
    let _ = std::fs::write(graphify_dir.join("graph_report.md"), &report);

    Ok(PipelineResultJs {
        nodes_added: 0,
        edges_added: 0,
        communities: cluster_result.communities.len() as i64,
        report,
    })
}

#[napi]
pub fn merge_graphs(root_a: String, root_b: String, out_root: String) -> napi::Result<PipelineResultJs> {
    let db_a = pipeline::load_graph_db(&PathBuf::from(&root_a))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db_b = pipeline::load_graph_db(&PathBuf::from(&root_b))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    // Create output directory and database
    let out_graphify = PathBuf::from(&out_root).join(".graphify");
    std::fs::create_dir_all(&out_graphify)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db = graphify_core::db::open_db(&out_graphify.join("db.sqlite"))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    // Collect all node IDs from A for dedup tracking
    let mut node_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut nodes_added = 0i64;

    // Insert nodes from A
    {
        let mut stmt = db_a.prepare(
            "SELECT id, label, file_type, source_file, source_line, docstring, community FROM nodes",
        ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, String, Option<i64>, Option<String>, Option<i64>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?
            .filter_map(|r| r.ok()).collect();
        for (id, label, ft, sf, line, doc, comm) in rows {
            db.execute(
                "INSERT OR IGNORE INTO nodes (id, label, file_type, source_file, source_line, docstring, community) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, label, ft, sf, line, doc, comm],
            ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
            node_ids.insert(id);
            nodes_added += 1;
        }
    }

    // Insert nodes from B (dedup by id)
    {
        let mut stmt = db_b.prepare(
            "SELECT id, label, file_type, source_file, source_line, docstring, community FROM nodes",
        ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, String, Option<i64>, Option<String>, Option<i64>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?
            .filter_map(|r| r.ok()).collect();
        for (id, label, ft, sf, line, doc, comm) in rows {
            if node_ids.contains(&id) { continue; }
            db.execute(
                "INSERT OR IGNORE INTO nodes (id, label, file_type, source_file, source_line, docstring, community) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, label, ft, sf, line, doc, comm],
            ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
            node_ids.insert(id);
            nodes_added += 1;
        }
    }

    // Insert edges from A
    let mut edges_added = 0i64;
    {
        let mut stmt = db_a.prepare(
            "SELECT source, target, relation, confidence, confidence_score, source_file FROM edges",
        ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let rows: Vec<(String, String, String, String, Option<f64>, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?
            .filter_map(|r| r.ok()).collect();
        for (src, tgt, rel, conf, score, sf) in rows {
            db.execute(
                "INSERT INTO edges (source, target, relation, confidence, confidence_score, source_file) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![src, tgt, rel, conf, score, sf],
            ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
            edges_added += 1;
        }
    }

    // Insert edges from B
    {
        let mut stmt = db_b.prepare(
            "SELECT source, target, relation, confidence, confidence_score, source_file FROM edges",
        ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let rows: Vec<(String, String, String, String, Option<f64>, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?
            .filter_map(|r| r.ok()).collect();
        for (src, tgt, rel, conf, score, sf) in rows {
            db.execute(
                "INSERT INTO edges (source, target, relation, confidence, confidence_score, source_file) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![src, tgt, rel, conf, score, sf],
            ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
            edges_added += 1;
        }
    }

    // Run cluster + analyze + report on merged graph
    let cluster_result = graphify_cluster::cluster(&db)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let analysis = graphify_analyze::analyze(&db)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let report = graphify_report::generate_report(&db, &analysis)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let _ = std::fs::write(out_graphify.join("graph_report.md"), &report);

    Ok(PipelineResultJs {
        nodes_added,
        edges_added,
        communities: cluster_result.communities.len() as i64,
        report,
    })
}

#[napi]
pub fn diff_graphs(root_a: String, root_b: String) -> napi::Result<DiffResultJs> {
    let db_a = pipeline::load_graph_db(&PathBuf::from(&root_a))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let db_b = pipeline::load_graph_db(&PathBuf::from(&root_b))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let ids_a: std::collections::HashSet<String> = collect_ids(&db_a, "SELECT id FROM nodes")
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let ids_b: std::collections::HashSet<String> = collect_ids(&db_b, "SELECT id FROM nodes")
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let added_ids: Vec<String> = ids_b.difference(&ids_a).cloned().collect();
    let removed_ids: Vec<String> = ids_a.difference(&ids_b).cloned().collect();

    let edges_a: std::collections::HashSet<(String, String, String)> = collect_edges(&db_a)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let edges_b: std::collections::HashSet<(String, String, String)> = collect_edges(&db_b)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let added_edges = edges_b.difference(&edges_a).count() as i64;
    let removed_edges = edges_a.difference(&edges_b).count() as i64;

    let added_labels = query_labels(&db_b, &added_ids)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let removed_labels = query_labels(&db_a, &removed_ids)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(DiffResultJs {
        nodes_added: added_ids.len() as i64,
        nodes_removed: removed_ids.len() as i64,
        edges_added: added_edges,
        edges_removed: removed_edges,
        added_node_labels: added_labels,
        removed_node_labels: removed_labels,
    })
}

fn collect_ids(db: &rusqlite::Connection, query: &str) -> graphify_core::Result<std::collections::HashSet<String>> {
    let mut stmt = db.prepare(query)?;
    let rows: Vec<String> = stmt.query_map([], |row| row.get(0))?.filter_map(|r| r.ok()).collect();
    Ok(rows.into_iter().collect())
}

fn collect_edges(db: &rusqlite::Connection) -> graphify_core::Result<std::collections::HashSet<(String, String, String)>> {
    let mut stmt = db.prepare("SELECT source, target, relation FROM edges")?;
    let rows: Vec<(String, String, String)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?.filter_map(|r| r.ok()).collect();
    Ok(rows.into_iter().collect())
}

fn query_labels(db: &rusqlite::Connection, ids: &[String]) -> graphify_core::Result<Vec<String>> {
    if ids.is_empty() { return Ok(Vec::new()); }
    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let q = format!("SELECT label FROM nodes WHERE id IN ({})", placeholders.join(","));
    let params: Vec<&dyn rusqlite::types::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
    let mut stmt = db.prepare(&q)?;
    let rows: Vec<String> = stmt.query_map(params.as_slice(), |row| row.get(0))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[napi]
pub fn graph_history(root: String, limit: i64) -> napi::Result<Vec<HistoryEntryJs>> {
    let db = pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let mut stmt = db.prepare(
        "SELECT id, question, answer, queried_at FROM query_history ORDER BY id DESC LIMIT ?1",
    ).map_err(|e| napi::Error::from_reason(e.to_string()))?;
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
