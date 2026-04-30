pub mod pipeline;

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
