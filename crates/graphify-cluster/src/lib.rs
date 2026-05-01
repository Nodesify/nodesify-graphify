// graphify-cluster: label propagation clustering

use petgraph::graph::NodeIndex;
use petgraph::graph::UnGraph;
use rusqlite::Connection;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ClusterResult {
    pub communities: HashMap<u32, usize>,
    pub iterations: u32,
}

pub fn cluster(db: &Connection) -> graphify_core::Result<ClusterResult> {
    // Load nodes
    let node_ids: Vec<String> = {
        let mut stmt = db.prepare("SELECT id FROM nodes")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    if node_ids.is_empty() {
        return Ok(ClusterResult {
            communities: HashMap::new(),
            iterations: 0,
        });
    }

    let id_to_idx: HashMap<String, NodeIndex> = node_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), NodeIndex::new(i)))
        .collect();

    let mut graph = UnGraph::<String, ()>::new_undirected();
    for id in &node_ids {
        graph.add_node(id.clone());
    }

    // Load edges
    {
        let mut stmt = db.prepare("SELECT source, target FROM edges")?;
        let edges: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        for (src, tgt) in edges {
            if let (Some(&s), Some(&t)) = (id_to_idx.get(&src), id_to_idx.get(&tgt)) {
                graph.add_edge(s, t, ());
            }
        }
    }

    // Label propagation
    let n = node_ids.len();
    let mut labels: Vec<u32> = (0..n as u32).collect();
    let mut iterations = 0;

    for _ in 0..100 {
        iterations += 1;
        let mut changed = false;

        for i in 0..n {
            let node_idx = NodeIndex::new(i);
            let mut neighbor_labels: HashMap<u32, usize> = HashMap::new();
            neighbor_labels.insert(labels[i], 1);

            for neighbor in graph.neighbors(node_idx) {
                *neighbor_labels.entry(labels[neighbor.index()]).or_insert(0) += 1;
            }

            let best_label = *neighbor_labels
                .iter()
                .max_by_key(|(_, &count)| count)
                .map(|(label, _)| label)
                .unwrap();

            if best_label != labels[i] {
                labels[i] = best_label;
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // Write communities back to SQLite
    for (i, id) in node_ids.iter().enumerate() {
        db.execute(
            "UPDATE nodes SET community = ?1 WHERE id = ?2",
            rusqlite::params![labels[i] as i64, id],
        )?;
    }

    let mut communities: HashMap<u32, usize> = HashMap::new();
    for &label in &labels {
        *communities.entry(label).or_insert(0) += 1;
    }

    Ok(ClusterResult {
        communities,
        iterations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;

    fn seed_graph(db: &Connection) {
        db.execute_batch("
            INSERT INTO nodes (id, label, file_type, source_file) VALUES ('a', 'A', 'code', 'f.py');
            INSERT INTO nodes (id, label, file_type, source_file) VALUES ('b', 'B', 'code', 'f.py');
            INSERT INTO nodes (id, label, file_type, source_file) VALUES ('c', 'C', 'code', 'f.py');
            INSERT INTO nodes (id, label, file_type, source_file) VALUES ('d', 'D', 'code', 'f.py');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'b', 'calls', 'EXTRACTED', 'f.py');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('b', 'c', 'calls', 'EXTRACTED', 'f.py');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('c', 'd', 'calls', 'EXTRACTED', 'f.py');
        ").unwrap();
    }

    #[test]
    fn cluster_assigns_communities() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db);
        let result = cluster(&db).unwrap();
        assert!(result.communities.len() >= 1);
        assert!(result.iterations > 0);
        let community: i64 = db
            .query_row("SELECT community FROM nodes WHERE id = 'a'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(community >= 0);
    }

    #[test]
    fn connected_graph_few_communities() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db);
        let result = cluster(&db).unwrap();
        // Label propagation on a chain can produce 1-2 communities depending on iteration order
        assert!(
            result.communities.len() <= 2,
            "expected at most 2 communities, got {}",
            result.communities.len()
        );
    }

    #[test]
    fn empty_graph_no_crash() {
        let db = open_db_in_memory().unwrap();
        let result = cluster(&db).unwrap();
        assert_eq!(result.communities.len(), 0);
    }
}
