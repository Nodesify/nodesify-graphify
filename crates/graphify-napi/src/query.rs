use std::collections::{HashMap, HashSet};

use petgraph::graph::{NodeIndex, UnGraph};
use rusqlite::Connection;

fn log_query(db: &Connection, question: &str, answer: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
    let _ = db.execute(
        "INSERT INTO query_history (question, answer, path_taken, queried_at) VALUES (?1, ?2, '', ?3)",
        rusqlite::params![question, answer.chars().take(500).collect::<String>(), ts],
    );
}

#[derive(Debug)]
struct NodeData {
    id: String,
    label: String,
    source_file: String,
    community: Option<i64>,
    docstring: Option<String>,
}

#[derive(Debug)]
struct EdgeData {
    relation: String,
    confidence: String,
}

struct LoadedGraph {
    graph: UnGraph<NodeData, EdgeData>,
    id_to_idx: HashMap<String, NodeIndex>,
}

fn load_graph(db: &Connection) -> graphify_core::Result<LoadedGraph> {
    let mut nodes = Vec::new();
    {
        let mut stmt = db.prepare(
            "SELECT id, label, source_file, community, docstring FROM nodes",
        )?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, Option<i64>, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
            .filter_map(|r| r.ok())
            .collect();
        for (id, label, sf, comm, doc) in rows {
            nodes.push((id, label, sf, comm, doc));
        }
    }

    let mut graph = UnGraph::new_undirected();
    let mut id_to_idx = HashMap::new();
    for (id, label, sf, comm, doc) in &nodes {
        let idx = graph.add_node(NodeData {
            id: id.clone(),
            label: label.clone(),
            source_file: sf.clone(),
            community: *comm,
            docstring: doc.clone(),
        });
        id_to_idx.insert(id.clone(), idx);
    }

    {
        let mut stmt = db.prepare(
            "SELECT source, target, relation, confidence FROM edges",
        )?;
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        for (src, tgt, rel, conf) in rows {
            if let (Some(&s), Some(&t)) = (id_to_idx.get(&src), id_to_idx.get(&tgt)) {
                graph.add_edge(s, t, EdgeData { relation: rel, confidence: conf });
            }
        }
    }

    Ok(LoadedGraph { graph, id_to_idx })
}

fn score_nodes(loaded: &LoadedGraph, terms: &[String]) -> Vec<(f64, NodeIndex)> {
    let mut scored: Vec<(f64, NodeIndex)> = Vec::new();
    for idx in loaded.graph.node_indices() {
        let node = &loaded.graph[idx];
        let label_lower = node.label.to_lowercase();
        let sf_lower = node.source_file.to_lowercase();
        let mut score = 0.0;
        for term in terms {
            if term.len() <= 2 { continue; }
            if label_lower.contains(&term.to_lowercase()) {
                score += 1.0;
            }
            if sf_lower.contains(&term.to_lowercase()) {
                score += 0.5;
            }
        }
        if score > 0.0 {
            scored.push((score, idx));
        }
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

fn bfs_subgraph(
    loaded: &LoadedGraph,
    start_nodes: &[NodeIndex],
    max_depth: usize,
) -> (HashSet<NodeIndex>, Vec<(NodeIndex, NodeIndex)>) {
    let mut visited: HashSet<NodeIndex> = start_nodes.iter().copied().collect();
    let mut frontier: Vec<NodeIndex> = start_nodes.to_vec();
    let mut edges_seen: Vec<(NodeIndex, NodeIndex)> = Vec::new();

    for _ in 0..max_depth {
        let mut next_frontier = Vec::new();
        for &node in &frontier {
            for neighbor in loaded.graph.neighbors(node) {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    next_frontier.push(neighbor);
                    edges_seen.push((node, neighbor));
                }
            }
        }
        if next_frontier.is_empty() { break; }
        frontier = next_frontier;
    }
    (visited, edges_seen)
}

fn dfs_subgraph(
    loaded: &LoadedGraph,
    start_nodes: &[NodeIndex],
    max_depth: usize,
) -> (HashSet<NodeIndex>, Vec<(NodeIndex, NodeIndex)>) {
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut edges_seen: Vec<(NodeIndex, NodeIndex)> = Vec::new();
    let mut stack: Vec<(NodeIndex, usize)> = start_nodes.iter().rev().map(|&n| (n, 0)).collect();

    while let Some((node, depth)) = stack.pop() {
        if visited.contains(&node) || depth > max_depth {
            continue;
        }
        visited.insert(node);
        for neighbor in loaded.graph.neighbors(node) {
            if !visited.contains(&neighbor) {
                stack.push((neighbor, depth + 1));
                edges_seen.push((node, neighbor));
            }
        }
    }
    (visited, edges_seen)
}

fn subgraph_to_text(
    loaded: &LoadedGraph,
    visited: &HashSet<NodeIndex>,
    edges_seen: &[(NodeIndex, NodeIndex)],
    token_budget: i64,
) -> String {
    let char_budget = (token_budget as usize) * 3;
    let mut out = String::new();

    let mut node_list: Vec<NodeIndex> = visited.iter().copied().collect();
    node_list.sort_by_key(|&idx| std::cmp::Reverse(loaded.graph.neighbors(idx).count()));

    for idx in &node_list {
        let node = &loaded.graph[*idx];
        let comm = node.community.map_or("?".to_string(), |c| c.to_string());
        let mut line = format!(
            "NODE {} [src={} community={}]\n",
            node.label, node.source_file, comm
        );
        if let Some(ref doc) = node.docstring {
            if !doc.is_empty() {
                let summary: String = doc.chars().take(200).collect();
                line.push_str(&format!("  summary: {}\n", summary));
            }
        }
        if out.len() + line.len() > char_budget {
            out.push_str(&format!("... (truncated to ~{} token budget)\n", token_budget));
            return out;
        }
        out.push_str(&line);
    }

    for (src_idx, tgt_idx) in edges_seen {
        let src = &loaded.graph[*src_idx];
        let tgt = &loaded.graph[*tgt_idx];
        if let Some(edge) = loaded.graph.edges_connecting(*src_idx, *tgt_idx).next() {
            let line = format!(
                "EDGE {} --{} [{}]--> {}\n",
                src.label, edge.weight().relation, edge.weight().confidence, tgt.label
            );
            if out.len() + line.len() > char_budget {
                out.push_str(&format!("... (truncated to ~{} token budget)\n", token_budget));
                return out;
            }
            out.push_str(&line);
        }
    }

    out
}

fn shortest_path_bfs(
    loaded: &LoadedGraph,
    start: NodeIndex,
    end: NodeIndex,
) -> Option<Vec<NodeIndex>> {
    if start == end {
        return Some(vec![start]);
    }
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        for neighbor in loaded.graph.neighbors(current) {
            if visited.contains(&neighbor) {
                continue;
            }
            parent.insert(neighbor, current);
            if neighbor == end {
                let mut path = vec![end];
                let mut cur = end;
                while let Some(&p) = parent.get(&cur) {
                    path.push(p);
                    cur = p;
                }
                path.reverse();
                return Some(path);
            }
            visited.insert(neighbor);
            queue.push_back(neighbor);
        }
    }
    None
}

pub fn query_graph(
    db: &Connection,
    question: &str,
    mode: &str,
    depth: usize,
    budget: i64,
) -> graphify_core::Result<(String, usize, usize)> {
    let loaded = load_graph(db)?;
    if loaded.graph.node_count() == 0 {
        return Ok(("No nodes in graph.".to_string(), 0, 0));
    }

    let terms: Vec<String> = question.split_whitespace().map(|s| s.to_string()).collect();
    let scored = score_nodes(&loaded, &terms);
    if scored.is_empty() {
        return Ok(("No matching nodes found.".to_string(), 0, 0));
    }

    let seed_nodes: Vec<NodeIndex> = scored.iter().take(5).map(|(_, idx)| *idx).collect();
    let (visited, edges_seen) = if mode == "dfs" {
        dfs_subgraph(&loaded, &seed_nodes, depth)
    } else {
        bfs_subgraph(&loaded, &seed_nodes, depth)
    };

    let seed_labels: Vec<String> = seed_nodes.iter()
        .map(|&idx| loaded.graph[idx].label.clone())
        .collect();

    let header = format!(
        "Traversal: {} depth={} | Start: {:?} | {} nodes found\n\n",
        mode.to_uppercase(), depth, seed_labels, visited.len()
    );
    let body = subgraph_to_text(&loaded, &visited, &edges_seen, budget);
    let result_text = header + &body;

    log_query(db, question, &result_text);

    Ok((result_text, visited.len(), edges_seen.len()))
}

pub fn find_shortest_path(
    db: &Connection,
    source_query: &str,
    target_query: &str,
) -> graphify_core::Result<(bool, usize, String)> {
    let loaded = load_graph(db)?;
    if loaded.graph.node_count() == 0 {
        return Ok((false, 0, "No nodes in graph.".to_string()));
    }

    let src_terms: Vec<String> = source_query.split_whitespace().map(|s| s.to_string()).collect();
    let tgt_terms: Vec<String> = target_query.split_whitespace().map(|s| s.to_string()).collect();

    let src_scored = score_nodes(&loaded, &src_terms);
    let tgt_scored = score_nodes(&loaded, &tgt_terms);

    let src_idx = match src_scored.first() {
        Some((_, idx)) => *idx,
        None => return Ok((false, 0, format!("No matching node for '{}'.", source_query))),
    };
    let tgt_idx = match tgt_scored.first() {
        Some((_, idx)) => *idx,
        None => return Ok((false, 0, format!("No matching node for '{}'.", target_query))),
    };

    let path = match shortest_path_bfs(&loaded, src_idx, tgt_idx) {
        Some(p) => p,
        None => return Ok((false, 0, "No path found.".to_string())),
    };

    let hops = path.len().saturating_sub(1);
    let mut text = format!("Shortest path ({} hops):\n", hops);

    for i in 0..path.len().saturating_sub(1) {
        let src = &loaded.graph[path[i]];
        let tgt = &loaded.graph[path[i + 1]];
        let edge_info = loaded.graph.edges_connecting(path[i], path[i + 1]).next();
        let rel = edge_info.map_or("?".to_string(), |e| e.weight().relation.clone());
        let conf = edge_info.map_or("?".to_string(), |e| e.weight().confidence.clone());
        text.push_str(&format!("  {} --{} [{}]--> {}\n", src.label, rel, conf, tgt.label));
    }

    let answer = format!("path found: {} hops", hops);
    log_query(db, &format!("{} -> {}", source_query, target_query), &answer);

    Ok((true, hops, text))
}

pub fn explain_with_neighbors(
    db: &Connection,
    node_id: &str,
) -> graphify_core::Result<Option<ExplainResult>> {
    let loaded = load_graph(db)?;

    let idx = match loaded.id_to_idx.get(node_id) {
        Some(&idx) => idx,
        None => {
            let terms: Vec<String> = node_id.split_whitespace().map(|s| s.to_string()).collect();
            let scored = score_nodes(&loaded, &terms);
            match scored.first() {
                Some((_, idx)) => *idx,
                None => return Ok(None),
            }
        }
    };

    let node = &loaded.graph[idx];
    let mut neighbors: Vec<EdgeInfoResult> = Vec::new();

    for neighbor in loaded.graph.neighbors(idx) {
        let neighbor_data = &loaded.graph[neighbor];
        let edge = loaded.graph.edges_connecting(idx, neighbor).next();
        neighbors.push(EdgeInfoResult {
            neighbor_id: neighbor_data.id.clone(),
            neighbor_label: neighbor_data.label.clone(),
            neighbor_file: neighbor_data.source_file.clone(),
            relation: edge.map_or("?".to_string(), |e| e.weight().relation.clone()),
            confidence: edge.map_or("?".to_string(), |e| e.weight().confidence.clone()),
        });
    }

    neighbors.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    neighbors.truncate(20);

    let answer = format!("explain: {} ({} neighbors)", node.label, loaded.graph.neighbors(idx).count());
    log_query(db, node_id, &answer);

    Ok(Some(ExplainResult {
        id: node.id.clone(),
        label: node.label.clone(),
        source_file: node.source_file.clone(),
        community: node.community,
        neighbor_count: loaded.graph.neighbors(idx).count(),
        neighbors,
    }))
}

pub struct EdgeInfoResult {
    pub neighbor_id: String,
    pub neighbor_label: String,
    pub neighbor_file: String,
    pub relation: String,
    pub confidence: String,
}

pub struct ExplainResult {
    pub id: String,
    pub label: String,
    pub source_file: String,
    pub community: Option<i64>,
    pub neighbor_count: usize,
    pub neighbors: Vec<EdgeInfoResult>,
}
