// graphify-report: markdown report generation

use graphify_analyze::AnalysisResult;
use rusqlite::Connection;

pub fn generate_report(
    db: &Connection,
    analysis: &AnalysisResult,
) -> graphify_core::Result<String> {
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

    let mut report = String::new();
    report.push_str("# Graph Report\n\n");
    report.push_str(&format!(
        "**Nodes:** {} | **Edges:** {} | **Communities:** {}\n\n",
        node_count, edge_count, community_count
    ));

    report.push_str("## Hub Nodes (God Nodes)\n\n");
    if analysis.god_nodes.is_empty() {
        report.push_str("No hub nodes found.\n\n");
    } else {
        for node in &analysis.god_nodes {
            let comm = node
                .community
                .map(|c| c.to_string())
                .unwrap_or_else(|| "—".into());
            report.push_str(&format!(
                "- **{}** (degree: {}, community: {})\n",
                node.label, node.degree, comm
            ));
        }
        report.push('\n');
    }

    report.push_str("## Surprising Connections\n\n");
    if analysis.surprising_connections.is_empty() {
        report.push_str("No cross-community connections found.\n\n");
    } else {
        for edge in &analysis.surprising_connections {
            let sc = edge
                .source_community
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".into());
            let tc = edge
                .target_community
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".into());
            report.push_str(&format!(
                "- {} -> {} ({}) [community {} -> {}]\n",
                edge.source, edge.target, edge.relation, sc, tc
            ));
        }
        report.push('\n');
    }

    report.push_str("## Suggested Questions\n\n");
    for q in &analysis.suggested_questions {
        report.push_str(&format!("- {}\n", q));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_analyze::{NodeAnalysis, SurprisingEdge};
    use graphify_core::db::open_db_in_memory;

    #[test]
    fn generate_report_with_data() {
        let db = open_db_in_memory().unwrap();
        db.execute_batch("
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('a', 'Alpha', 'code', 'f.py', 0);
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('b', 'Beta', 'code', 'f.py', 1);
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'b', 'calls', 'EXTRACTED', 'f.py');
        ").unwrap();

        let analysis = AnalysisResult {
            god_nodes: vec![NodeAnalysis {
                id: "a".into(),
                label: "Alpha".into(),
                degree: 1,
                community: Some(0),
            }],
            surprising_connections: vec![SurprisingEdge {
                source: "a".into(),
                target: "b".into(),
                relation: "calls".into(),
                source_community: Some(0),
                target_community: Some(1),
            }],
            suggested_questions: vec!["Why does Alpha have so many connections?".into()],
        };

        let report = generate_report(&db, &analysis).unwrap();
        assert!(report.contains("# Graph Report"));
        assert!(report.contains("Alpha"));
        assert!(report.contains("Surprising Connections"));
        assert!(report.contains("Suggested Questions"));
    }

    #[test]
    fn generate_report_empty_graph() {
        let db = open_db_in_memory().unwrap();
        let analysis = AnalysisResult {
            god_nodes: vec![],
            surprising_connections: vec![],
            suggested_questions: vec![],
        };
        let report = generate_report(&db, &analysis).unwrap();
        assert!(report.contains("**Nodes:** 0"));
    }
}
