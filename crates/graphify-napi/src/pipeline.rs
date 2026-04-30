use std::path::{Path, PathBuf};

use graphify_core::db;
use rusqlite::Connection;

#[derive(Debug)]
pub struct PipelineResult {
    pub build_result: graphify_build::BuildResult,
    pub cluster_result: graphify_cluster::ClusterResult,
    pub analysis: graphify_analyze::AnalysisResult,
    pub report: String,
}

pub fn run_pipeline(root: &Path) -> graphify_core::Result<PipelineResult> {
    let graphify_dir = root.join(".graphify");
    std::fs::create_dir_all(&graphify_dir)?;

    let db_path = graphify_dir.join("db.sqlite");
    let db = db::open_db(&db_path)?;

    let detected = graphify_detect::detect(root, &db)?;
    graphify_detect::update_manifest(&detected, &db)?;

    let files_to_process: Vec<PathBuf> = detected
        .new
        .iter()
        .chain(detected.changed.iter())
        .map(|e| root.join(&e.path))
        .collect();

    if files_to_process.is_empty() && detected.removed.is_empty() {
        let analysis = graphify_analyze::analyze(&db)?;
        let report = graphify_report::generate_report(&db, &analysis)?;
        write_report(&graphify_dir, &report);
        export_json(&db, &graphify_dir.join("graph.json"))?;
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

    let extractions = graphify_extract::extract(&files_to_process, &db)?;
    let build_result = graphify_build::build(&extractions, &db)?;
    let cluster_result = graphify_cluster::cluster(&db)?;
    let analysis = graphify_analyze::analyze(&db)?;
    let report = graphify_report::generate_report(&db, &analysis)?;

    write_report(&graphify_dir, &report);
    export_json(&db, &graphify_dir.join("graph.json"))?;

    Ok(PipelineResult {
        build_result,
        cluster_result,
        analysis,
        report,
    })
}

fn write_report(graphify_dir: &Path, report: &str) {
    let _ = std::fs::write(graphify_dir.join("graph_report.md"), report);
}

pub fn export_json(db: &Connection, out_path: &Path) -> graphify_core::Result<()> {
    let mut nodes = Vec::new();
    let mut stmt = db.prepare(
        "SELECT id, label, file_type, source_file, source_line, docstring, community FROM nodes",
    )?;
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
    db::open_db(&root.join(".graphify").join("db.sqlite"))
}
