use std::path::{Path, PathBuf};
use std::collections::HashMap;

use graphify_core::db;
use graphify_paths;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct PipelineResult {
    pub build_result: graphify_build::BuildResult,
    pub cluster_result: graphify_cluster::ClusterResult,
    pub analysis: graphify_analyze::AnalysisResult,
    pub report: String,
}

fn timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn semantic_cache_key(path: &Path) -> String {
    format!("semantic:{}", graphify_paths::normalize(path))
}

fn file_hash(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(format!("{:x}", hasher.finalize()))
}

fn check_semantic_cache(
    db: &Connection,
    path: &Path,
    hash: &str,
) -> Option<graphify_semantic::SemanticExtraction> {
    let key = semantic_cache_key(path);
    let mut stmt = db
        .prepare(
            "SELECT nodes, edges FROM extraction_cache WHERE file_path = ?1 AND content_hash = ?2",
        )
        .ok()?;
    stmt.query_row(rusqlite::params![&key, hash], |row| {
        let nodes_json: String = row.get(0)?;
        let edges_json: String = row.get(1)?;
        Ok((nodes_json, edges_json))
    })
    .ok()
    .map(|(nodes_json, edges_json)| {
        let nodes: Vec<graphify_semantic::SemanticNode> =
            serde_json::from_str(&nodes_json).unwrap_or_default();
        let edges: Vec<graphify_semantic::SemanticEdge> =
            serde_json::from_str(&edges_json).unwrap_or_default();
        graphify_semantic::SemanticExtraction { nodes, edges }
    })
}

fn save_semantic_cache(
    db: &Connection,
    path: &Path,
    hash: &str,
    extraction: &graphify_semantic::SemanticExtraction,
) {
    let key = semantic_cache_key(path);
    let nodes_json = serde_json::to_string(&extraction.nodes).unwrap_or_default();
    let edges_json = serde_json::to_string(&extraction.edges).unwrap_or_default();
    let now = timestamp();
    let _ = db.execute(
        "INSERT OR REPLACE INTO extraction_cache (file_path, content_hash, language, nodes, edges, extracted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![&key, hash, "semantic", nodes_json, edges_json, now],
    );
}

/// Enrich existing extractions with LLM-based semantic data.
/// No-op if GRAPHIFY_LLM_API_KEY is not set.
fn enrich_with_semantics(
    files: &[PathBuf],
    extractions: &mut [graphify_extract::Extraction],
    db: &Connection,
) {
    let backend = match graphify_semantic::ClaudeBackend::from_env() {
        Ok(b) => b,
        Err(_) => return,
    };

    let mut file_to_idx: HashMap<PathBuf, usize> = HashMap::new();
    for (i, ext) in extractions.iter().enumerate() {
        file_to_idx.insert(ext.file_path.clone(), i);
    }

    for file_path in files {
        let Some(&idx) = file_to_idx.get(file_path) else {
            continue;
        };
        let hash = match file_hash(file_path) {
            Some(h) => h,
            None => continue,
        };

        let sem_ext = match check_semantic_cache(db, file_path, &hash) {
            Some(cached) => cached,
            None => {
                let results = graphify_semantic::extract_semantic_for_files(
                    std::slice::from_ref(file_path),
                    &backend,
                );
                let Some((_, extraction)) = results.into_iter().next() else {
                    continue;
                };
                save_semantic_cache(db, file_path, &hash, &extraction);
                extraction
            }
        };

        let ext = &mut extractions[idx];
        for sem_node in sem_ext.nodes {
            ext.nodes.push(graphify_extract::ExtractedNode {
                id: sem_node.id,
                label: sem_node.label,
                source_file: file_path.clone(),
                source_line: None,
                docstring: Some(sem_node.summary),
                node_type: sem_node.node_type,
            });
        }
        for sem_edge in sem_ext.edges {
            ext.edges.push(graphify_extract::ExtractedEdge {
                source: sem_edge.source,
                target: sem_edge.target,
                relation: sem_edge.relation,
                confidence: "SEMANTIC".to_string(),
                confidence_score: None,
                source_file: file_path.clone(),
                source_line: None,
            });
        }
    }
}

pub fn run_pipeline(root: &Path) -> graphify_core::Result<PipelineResult> {
    let root = if root.exists() {
        root.canonicalize().map_err(graphify_core::GraphifyError::Io)?
    } else {
        return Err(graphify_core::GraphifyError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path does not exist: {}", root.display()),
        )));
    };
    let graphify_dir = graphify_paths::graphify_dir(&root)?;
    let db_path = graphify_paths::db_path(&root)?;
    let db = db::open_db(&db_path)?;

    // Record pipeline start (root is now canonicalized)
    let run_id: i64 = db.query_row(
        "INSERT INTO pipeline_runs (started_at, status) VALUES (?1, 'running') RETURNING id",
        rusqlite::params![timestamp()],
        |row| row.get(0),
    )?;

    let result = run_pipeline_inner(&root, &db, &graphify_dir);

    // Record pipeline completion
    let (status, files_processed, nodes_added, edges_added) = match &result {
        Ok(r) => ("completed", 0i64, r.build_result.nodes_added as i64, r.build_result.edges_added as i64),
        Err(_) => ("failed", 0, 0, 0),
    };
    if let Err(e) = db.execute(
        "UPDATE pipeline_runs SET finished_at = ?1, status = ?2, files_processed = ?3, nodes_added = ?4, edges_added = ?5 WHERE id = ?6",
        rusqlite::params![timestamp(), status, files_processed, nodes_added, edges_added, run_id],
    ) {
        eprintln!("warning: failed to record pipeline status: {}", e);
    }

    result
}

fn run_pipeline_inner(root: &Path, db: &Connection, graphify_dir: &Path) -> graphify_core::Result<PipelineResult> {
    let detected = graphify_detect::detect(root, db)?;
    graphify_detect::update_manifest(&detected, db)?;

    // Clean up removed files from the graph
    for entry in &detected.removed {
        let path_str = graphify_paths::normalize(&entry.path);
        if let Err(e) = db.execute("DELETE FROM edges WHERE source_file = ?1", rusqlite::params![path_str]) {
            eprintln!("warning: failed to clean edges for {}: {}", path_str, e);
        }
        if let Err(e) = db.execute("DELETE FROM nodes WHERE source_file = ?1", rusqlite::params![path_str]) {
            eprintln!("warning: failed to clean nodes for {}: {}", path_str, e);
        }
        if let Err(e) = db.execute("DELETE FROM extraction_cache WHERE file_path = ?1", rusqlite::params![path_str]) {
            eprintln!("warning: failed to clean cache for {}: {}", path_str, e);
        }
        let semantic_key = format!("semantic:{}", path_str);
        if let Err(e) = db.execute("DELETE FROM extraction_cache WHERE file_path = ?1", rusqlite::params![semantic_key]) {
            eprintln!("warning: failed to clean semantic cache for {}: {}", path_str, e);
        }
    }

    let files_to_process: Vec<PathBuf> = detected
        .new
        .iter()
        .chain(detected.changed.iter())
        .map(|e| root.join(&e.path))
        .collect();

    if files_to_process.is_empty() && detected.removed.is_empty() {
        let analysis = graphify_analyze::analyze(db)?;
        let report = graphify_report::generate_report(db, &analysis)?;
        write_report(graphify_dir, &report)?;
        export_json(db, &graphify_dir.join("graph.json"))?;
        return Ok(PipelineResult {
            build_result: graphify_build::BuildResult {
                nodes_added: 0,
                edges_added: 0,
                duplicates_merged: 0,
            },
            cluster_result: graphify_cluster::ClusterResult {
                communities: Default::default(),
                iterations: 0,
            },
            analysis,
            report,
        });
    }

    let mut extractions = graphify_extract::extract(&files_to_process, db)?;
    enrich_with_semantics(&files_to_process, &mut extractions, db);
    let build_result = graphify_build::build(&extractions, db)?;
    let cluster_result = graphify_cluster::cluster(db)?;
    let analysis = graphify_analyze::analyze(db)?;
    let report = graphify_report::generate_report(db, &analysis)?;

    write_report(graphify_dir, &report)?;
    export_json(db, &graphify_dir.join("graph.json"))?;

    Ok(PipelineResult {
        build_result,
        cluster_result,
        analysis,
        report,
    })
}

fn write_report(graphify_dir: &Path, report: &str) -> graphify_core::Result<()> {
    std::fs::write(graphify_dir.join("graph_report.md"), report)?;
    Ok(())
}

pub fn export_json(db: &Connection, out_path: &Path) -> graphify_core::Result<()> {
    let mut nodes = Vec::new();
    let mut stmt = db.prepare(
        "SELECT id, label, file_type, source_file, source_line, docstring, community FROM nodes",
    )?;
    #[allow(clippy::type_complexity)]
    let node_rows: Vec<(String, String, String, String, Option<i64>, Option<String>, Option<i64>)> =
        stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    for (id, label, ft, sf, line, doc, comm) in &node_rows {
        nodes.push(serde_json::json!({
            "id": id,
            "label": label,
            "file_type": ft,
            "source_file": sf,
            "source_line": line,
            "docstring": doc,
            "community": comm,
        }));
    }

    let mut edges = Vec::new();
    let mut stmt = db.prepare(
        "SELECT source, target, relation, confidence, confidence_score, source_file FROM edges",
    )?;
    let edge_rows: Vec<(String, String, String, String, Option<f64>, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    for (src, tgt, rel, conf, score, sf) in &edge_rows {
        edges.push(serde_json::json!({
            "source": src,
            "target": tgt,
            "relation": rel,
            "confidence": conf,
            "confidence_score": score,
            "source_file": sf,
        }));
    }

    let graph = serde_json::json!({ "nodes": nodes, "edges": edges });
    let json = serde_json::to_string_pretty(&graph)?;
    std::fs::write(out_path, json)?;
    Ok(())
}

pub fn load_graph_db(root: &Path) -> graphify_core::Result<Connection> {
    let p = graphify_paths::db_path(root)?;
    db::open_db(&p)
}
