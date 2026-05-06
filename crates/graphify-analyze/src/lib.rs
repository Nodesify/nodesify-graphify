// graphify-analyze: god nodes, surprises, and question detection

use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct NodeAnalysis {
    pub id: String,
    pub label: String,
    pub degree: usize,
    pub community: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SurprisingEdge {
    pub source: String,
    pub source_label: String,
    pub target: String,
    pub target_label: String,
    pub relation: String,
    pub source_community: Option<u32>,
    pub target_community: Option<u32>,
}

#[derive(Debug)]
pub struct AnalysisResult {
    pub god_nodes: Vec<NodeAnalysis>,
    pub surprising_connections: Vec<SurprisingEdge>,
    pub suggested_questions: Vec<String>,
}

pub fn analyze(db: &Connection) -> graphify_core::Result<AnalysisResult> {
    let god_nodes = compute_god_nodes(db)?;
    let surprising = compute_surprising_connections(db)?;
    let questions = suggest_questions(db, &god_nodes)?;
    Ok(AnalysisResult {
        god_nodes,
        surprising_connections: surprising,
        suggested_questions: questions,
    })
}

fn compute_god_nodes(db: &Connection) -> graphify_core::Result<Vec<NodeAnalysis>> {
    let mut stmt = db.prepare(
        "SELECT n.id, n.label, n.community, COUNT(e.id) as degree
         FROM nodes n LEFT JOIN edges e ON e.source = n.id OR e.target = n.id
         GROUP BY n.id ORDER BY degree DESC LIMIT 10",
    )?;
    let nodes: Vec<NodeAnalysis> = stmt
        .query_map([], |row| {
            Ok(NodeAnalysis {
                id: row.get(0)?,
                label: row.get(1)?,
                community: row.get::<_, Option<i64>>(2)?.map(|c| c as u32),
                degree: row.get::<_, i64>(3)? as usize,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    if !nodes.is_empty() {
        let max_degree = nodes[0].degree as f64;
        if max_degree > 0.0 {
            for node in &nodes {
                let centrality = node.degree as f64 / max_degree;
                db.execute(
                    "UPDATE nodes SET degree_centrality = ?1 WHERE id = ?2",
                    rusqlite::params![centrality, node.id],
                )?;
            }
        }
    }
    Ok(nodes)
}

fn compute_surprising_connections(db: &Connection) -> graphify_core::Result<Vec<SurprisingEdge>> {
    let mut stmt = db.prepare(
        "SELECT e.source, s.label, e.target, t.label, e.relation, s.community, t.community
         FROM edges e
         JOIN nodes s ON s.id = e.source JOIN nodes t ON t.id = e.target
         WHERE s.community IS NOT NULL AND t.community IS NOT NULL AND s.community != t.community",
    )?;
    let edges: Vec<SurprisingEdge> = stmt
        .query_map([], |row| {
            Ok(SurprisingEdge {
                source: row.get(0)?,
                source_label: row.get(1)?,
                target: row.get(2)?,
                target_label: row.get(3)?,
                relation: row.get(4)?,
                source_community: row.get::<_, Option<i64>>(5)?.map(|c| c as u32),
                target_community: row.get::<_, Option<i64>>(6)?.map(|c| c as u32),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(edges)
}

fn suggest_questions(
    db: &Connection,
    god_nodes: &[NodeAnalysis],
) -> graphify_core::Result<Vec<String>> {
    let mut questions = Vec::new();
    for node in god_nodes.iter().take(5) {
        questions.push(format!("Why does {} have so many connections?", node.label));
    }
    let community_count: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT community) FROM nodes WHERE community IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if community_count > 1 {
        questions.push(format!(
            "What connects the {} different communities?",
            community_count
        ));
    }
    Ok(questions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;

    fn seed_analyzed_graph(db: &Connection) {
        db.execute_batch(
            "
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('a', 'Alpha', 'code', 'f.py', 0);
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('b', 'Beta', 'code', 'f.py', 0);
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('c', 'Gamma', 'code', 'f.py', 1);
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'b', 'calls', 'EXTRACTED', 'f.py');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'c', 'calls', 'EXTRACTED', 'f.py');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('b', 'c', 'calls', 'EXTRACTED', 'f.py');
        ",
        )
        .unwrap();
    }

    #[test]
    fn analyze_finds_god_nodes() {
        let db = open_db_in_memory().unwrap();
        seed_analyzed_graph(&db);
        let result = analyze(&db).unwrap();
        assert!(!result.god_nodes.is_empty());
        assert_eq!(result.god_nodes[0].id, "a");
    }

    #[test]
    fn analyze_finds_surprising_connections() {
        let db = open_db_in_memory().unwrap();
        seed_analyzed_graph(&db);
        let result = analyze(&db).unwrap();
        assert!(!result.surprising_connections.is_empty());
    }

    #[test]
    fn analyze_suggests_questions() {
        let db = open_db_in_memory().unwrap();
        seed_analyzed_graph(&db);
        let result = analyze(&db).unwrap();
        assert!(!result.suggested_questions.is_empty());
    }
}
