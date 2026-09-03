use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, RwLock};

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use rusqlite::Connection;

use graphify_paths::relative_display;

/// Fraction of the output budget reserved for EDGE lines. Nodes alone are
/// capped at this share so a traversal never returns a bag of labels with
/// the relationships truncated away.
const NODE_BUDGET_SHARE: f64 = 0.6;
/// How many near-miss labels to suggest when a query matches nothing.
const SUGGESTION_COUNT: usize = 3;

/// Global cache of loaded graphs, keyed by normalized DB path.
static GRAPH_CACHE: LazyLock<RwLock<HashMap<String, Arc<LoadedGraph>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Invalidate the cached graph for a given DB path. Called after pipeline runs.
pub fn invalidate_graph_cache(db_path: &str) {
    let mut cache = GRAPH_CACHE.write().unwrap();
    cache.remove(db_path);
}

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
    confidence_score: Option<f64>,
}

impl EdgeData {
    /// Effective strength of this edge: the stored numeric score when
    /// present, otherwise a rank derived from the confidence label.
    fn strength(&self) -> f64 {
        self.confidence_score
            .unwrap_or_else(|| confidence_rank(&self.confidence))
    }
}

/// Fallback strength for edges without a numeric score. Alphabetical
/// string comparison of confidence labels does NOT order by strength
/// ("SEMANTIC" > "LLM" lexicographically), so map labels to numbers.
fn confidence_rank(confidence: &str) -> f64 {
    match confidence.to_uppercase().as_str() {
        "DECLARED" => 1.0,
        "EXTRACTED" => 0.9,
        "INFERRED" => 0.7,
        "SEMANTIC" => 0.6,
        _ => 0.5,
    }
}

struct LoadedGraph {
    /// Directed storage even though most traversals are undirected: the
    /// edge orientation (caller → callee, importer → module) is preserved,
    /// and `directed` queries can follow it.
    graph: DiGraph<NodeData, EdgeData>,
    id_to_idx: HashMap<String, NodeIndex>,
    /// Project root derived from the DB path (`root/.graphify/db.sqlite`),
    /// used to shorten stored absolute paths in agent-facing output.
    root: Option<String>,
}

impl LoadedGraph {
    /// Root-relative display form of a stored path.
    fn display_path(&self, path: &str) -> String {
        match &self.root {
            Some(root) => relative_display(path, root),
            None => path.trim_start_matches("//?/").to_string(),
        }
    }
}

fn load_graph(db: &Connection, db_path: &str) -> graphify_core::Result<LoadedGraph> {
    // Project root: two levels above the DB file (root/.graphify/db.sqlite).
    let root = std::path::Path::new(db_path)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().replace('\\', "/"));

    let mut nodes = Vec::new();
    {
        let mut stmt =
            db.prepare("SELECT id, label, source_file, community, docstring FROM nodes")?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, Option<i64>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        for (id, label, sf, comm, doc) in rows {
            nodes.push((id, label, sf, comm, doc));
        }
    }

    let mut graph = DiGraph::new();
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
        let mut stmt =
            db.prepare("SELECT source, target, relation, confidence, confidence_score FROM edges")?;
        let rows: Vec<(String, String, String, String, Option<f64>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        for (src, tgt, rel, conf, score) in rows {
            if let (Some(&s), Some(&t)) = (id_to_idx.get(&src), id_to_idx.get(&tgt)) {
                graph.add_edge(
                    s,
                    t,
                    EdgeData {
                        relation: rel,
                        confidence: conf,
                        confidence_score: score,
                    },
                );
            }
        }
    }

    Ok(LoadedGraph {
        graph,
        id_to_idx,
        root,
    })
}

fn load_graph_cached(db: &Connection, db_path: &str) -> graphify_core::Result<Arc<LoadedGraph>> {
    {
        let cache = GRAPH_CACHE.read().unwrap();
        if let Some(entry) = cache.get(db_path) {
            return Ok(Arc::clone(entry));
        }
    }

    let loaded = Arc::new(load_graph(db, db_path)?);
    {
        let mut cache = GRAPH_CACHE.write().unwrap();
        cache.insert(db_path.to_string(), Arc::clone(&loaded));
    }
    Ok(loaded)
}

/// Neighbors of `idx`: outgoing only when traversing a directed graph,
/// both directions otherwise.
fn iter_neighbors<'a>(
    graph: &'a DiGraph<NodeData, EdgeData>,
    idx: NodeIndex,
    directed: bool,
) -> impl Iterator<Item = NodeIndex> + 'a {
    let outgoing = graph.neighbors_directed(idx, Direction::Outgoing);
    if directed {
        Box::new(outgoing) as Box<dyn Iterator<Item = NodeIndex> + 'a>
    } else {
        Box::new(outgoing.chain(graph.neighbors_directed(idx, Direction::Incoming)))
            as Box<dyn Iterator<Item = NodeIndex> + 'a>
    }
}

/// The strongest edge connecting `a` and `b`, in either direction.
fn edge_between(
    graph: &DiGraph<NodeData, EdgeData>,
    a: NodeIndex,
    b: NodeIndex,
) -> Option<&EdgeData> {
    let forward = graph
        .edges_directed(a, Direction::Outgoing)
        .find(|e| e.target() == b)
        .map(|e| e.weight());
    match forward {
        Some(w) => Some(w),
        None => graph
            .edges_directed(b, Direction::Outgoing)
            .find(|e| e.target() == a)
            .map(|e| e.weight()),
    }
}

/// Lowercase word tokens, splitting camelCase / snake_case / kebab-case and
/// punctuation so "parseExtraction", "parse_extraction" and
/// "parse-extraction" all tokenize identically.
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            let prev_upper = current.chars().last().is_some_and(|c| c.is_uppercase());
            if !current.is_empty() && ch.is_uppercase() && !prev_upper {
                tokens.push(std::mem::take(&mut current).to_lowercase());
            }
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current).to_lowercase());
        }
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }
    tokens
}

/// Score nodes against question terms: exact token match on the label beats
/// prefix beats substring; file-path matches count at half weight.
/// The `k` node labels most similar to `query` — did-you-mean suggestions
/// so a failed lookup hands the agent something actionable instead of a
/// dead end. Best Jaro-Winkler score across the query's terms wins.
fn nearest_labels(loaded: &LoadedGraph, query: &str, k: usize) -> Vec<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter(|t| t.len() > 2)
        .map(|t| t.to_lowercase())
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(f64, &String)> = Vec::new();
    for idx in loaded.graph.node_indices() {
        let label = &loaded.graph[idx].label;
        let label_lower = label.to_lowercase();
        let best = terms
            .iter()
            .map(|t| strsim::jaro_winkler(&label_lower, t))
            .fold(0.0_f64, f64::max);
        if best > 0.6 {
            scored.push((best, label));
        }
    }
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(b.1))
    });
    scored.truncate(k);
    scored.into_iter().map(|(_, l)| l.clone()).collect()
}

fn score_nodes(loaded: &LoadedGraph, terms: &[String]) -> Vec<(f64, NodeIndex)> {
    let mut scored: Vec<(f64, NodeIndex)> = Vec::new();
    for idx in loaded.graph.node_indices() {
        let node = &loaded.graph[idx];
        let label_tokens = tokenize(&node.label);
        let file_tokens = tokenize(&node.source_file);
        let mut score = 0.0;
        for term in terms {
            let term = term.trim();
            if term.len() <= 2 {
                continue;
            }
            let term_lower = term.to_lowercase();
            let term_tokens = tokenize(term);
            let label_lower = node.label.to_lowercase();
            let sf_lower = node.source_file.to_lowercase();

            let label_score = if label_lower.contains(&term_lower) {
                1.0
            } else {
                term_tokens
                    .iter()
                    .map(|tt| {
                        if label_tokens.iter().any(|lt| lt == tt) {
                            0.9
                        } else if label_tokens.iter().any(|lt| lt.starts_with(tt.as_str())) {
                            0.7
                        } else if label_tokens.iter().any(|lt| lt.contains(tt.as_str())) {
                            0.5
                        } else {
                            0.0
                        }
                    })
                    .fold(0.0_f64, f64::max)
            };
            if label_score > 0.0 {
                score += label_score;
                continue;
            }
            if sf_lower.contains(&term_lower)
                || file_tokens.iter().any(|ft| term_tokens.contains(ft))
            {
                score += 0.5;
            }
        }
        if score > 0.0 {
            scored.push((score, idx));
        }
    }
    // Deterministic order: score desc, then label, then id.
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| loaded.graph[a.1].label.cmp(&loaded.graph[b.1].label))
            .then_with(|| loaded.graph[a.1].id.cmp(&loaded.graph[b.1].id))
    });
    scored
}

fn bfs_subgraph(
    loaded: &LoadedGraph,
    start_nodes: &[NodeIndex],
    max_depth: usize,
    directed: bool,
) -> (HashSet<NodeIndex>, Vec<(NodeIndex, NodeIndex)>) {
    let mut visited: HashSet<NodeIndex> = start_nodes.iter().copied().collect();
    let mut frontier: Vec<NodeIndex> = start_nodes.to_vec();
    let mut edges_seen: Vec<(NodeIndex, NodeIndex)> = Vec::new();

    for _ in 0..max_depth {
        let mut next_frontier = Vec::new();
        for &node in &frontier {
            for neighbor in iter_neighbors(&loaded.graph, node, directed) {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    next_frontier.push(neighbor);
                    edges_seen.push((node, neighbor));
                }
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        frontier = next_frontier;
    }
    (visited, edges_seen)
}

fn dfs_subgraph(
    loaded: &LoadedGraph,
    start_nodes: &[NodeIndex],
    max_depth: usize,
    directed: bool,
) -> (HashSet<NodeIndex>, Vec<(NodeIndex, NodeIndex)>) {
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut edges_seen: Vec<(NodeIndex, NodeIndex)> = Vec::new();
    let mut stack: Vec<(NodeIndex, usize)> = start_nodes.iter().rev().map(|&n| (n, 0)).collect();

    while let Some((node, depth)) = stack.pop() {
        if visited.contains(&node) || depth > max_depth {
            continue;
        }
        visited.insert(node);
        for neighbor in iter_neighbors(&loaded.graph, node, directed) {
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
    // Nodes alone may not consume more than their share — the relationships
    // are the point of a graph traversal, so edges always keep budget.
    let node_budget = (char_budget as f64 * NODE_BUDGET_SHARE) as usize;
    let mut out = String::new();

    let mut node_list: Vec<NodeIndex> = visited.iter().copied().collect();
    node_list.sort_by_key(|&idx| std::cmp::Reverse(loaded.graph.neighbors(idx).count()));

    let mut shown_nodes = 0usize;
    for idx in &node_list {
        let node = &loaded.graph[*idx];
        let comm = node.community.map_or("?".to_string(), |c| c.to_string());
        let mut line = format!(
            "NODE {} [id={} src={} community={}]\n",
            node.label,
            node.id,
            loaded.display_path(&node.source_file),
            comm
        );
        if let Some(ref doc) = node.docstring {
            if !doc.is_empty() {
                let summary: String = doc.chars().take(200).collect();
                line.push_str(&format!("  summary: {}\n", summary));
            }
        }
        if out.len() + line.len() > node_budget && shown_nodes > 0 {
            out.push_str(&format!(
                "... (showing top {} of {} nodes; edges follow)\n",
                shown_nodes,
                node_list.len()
            ));
            break;
        }
        out.push_str(&line);
        shown_nodes += 1;
    }

    // Edges get the remaining budget, with a floor so even a tiny budget
    // still yields some relationships instead of node labels only.
    let edge_budget = char_budget.saturating_sub(node_budget).max(200);
    let mut edge_spent = 0usize;
    for (src_idx, tgt_idx) in edges_seen {
        let src = &loaded.graph[*src_idx];
        let tgt = &loaded.graph[*tgt_idx];
        if let Some(edge) = edge_between(&loaded.graph, *src_idx, *tgt_idx) {
            let line = format!(
                "EDGE {} --{} [{}]--> {}\n",
                src.label, edge.relation, edge.confidence, tgt.label
            );
            if edge_spent > 0 && edge_spent + line.len() > edge_budget {
                out.push_str(&format!(
                    "... (truncated to ~{} token budget)\n",
                    token_budget
                ));
                return out;
            }
            out.push_str(&line);
            edge_spent += line.len();
        }
    }

    out
}

fn shortest_path_bfs(
    loaded: &LoadedGraph,
    start: NodeIndex,
    end: NodeIndex,
    directed: bool,
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
        for neighbor in iter_neighbors(&loaded.graph, current, directed) {
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

#[allow(clippy::too_many_arguments)]
pub fn query_graph(
    db: &Connection,
    db_path: &str,
    question: &str,
    mode: &str,
    depth: usize,
    budget: i64,
    directed: bool,
) -> graphify_core::Result<(String, usize, usize)> {
    let loaded = load_graph_cached(db, db_path)?;
    if loaded.graph.node_count() == 0 {
        return Ok(("No nodes in graph.".to_string(), 0, 0));
    }

    let terms: Vec<String> = question.split_whitespace().map(|s| s.to_string()).collect();
    let scored = score_nodes(&loaded, &terms);
    if scored.is_empty() {
        let suggestions = nearest_labels(&loaded, question, SUGGESTION_COUNT);
        let msg = if suggestions.is_empty() {
            "No matching nodes found.".to_string()
        } else {
            format!(
                "No matching nodes found. Did you mean: {}?",
                suggestions.join(", ")
            )
        };
        return Ok((msg, 0, 0));
    }

    let seed_nodes: Vec<NodeIndex> = scored.iter().take(5).map(|(_, idx)| *idx).collect();
    let (visited, edges_seen) = if mode == "dfs" {
        dfs_subgraph(&loaded, &seed_nodes, depth, directed)
    } else {
        bfs_subgraph(&loaded, &seed_nodes, depth, directed)
    };

    let seed_labels: Vec<String> = seed_nodes
        .iter()
        .map(|&idx| loaded.graph[idx].label.clone())
        .collect();

    let header = format!(
        "Traversal: {} depth={}{} | Start: {:?} | {} nodes found\n\n",
        mode.to_uppercase(),
        depth,
        if directed { " directed" } else { "" },
        seed_labels,
        visited.len()
    );
    let body = subgraph_to_text(&loaded, &visited, &edges_seen, budget);
    let result_text = header + &body;

    log_query(db, question, &result_text);

    Ok((result_text, visited.len(), edges_seen.len()))
}

pub fn find_shortest_path(
    db: &Connection,
    db_path: &str,
    source_query: &str,
    target_query: &str,
    directed: bool,
) -> graphify_core::Result<(bool, usize, String)> {
    let loaded = load_graph_cached(db, db_path)?;
    if loaded.graph.node_count() == 0 {
        return Ok((false, 0, "No nodes in graph.".to_string()));
    }

    let src_terms: Vec<String> = source_query
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let tgt_terms: Vec<String> = target_query
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let src_scored = score_nodes(&loaded, &src_terms);
    let tgt_scored = score_nodes(&loaded, &tgt_terms);

    let src_idx = match src_scored.first() {
        Some((_, idx)) => *idx,
        None => {
            let mut msg = format!("No matching node for '{}'.", source_query);
            let suggestions = nearest_labels(&loaded, source_query, SUGGESTION_COUNT);
            if !suggestions.is_empty() {
                msg.push_str(&format!(" Did you mean: {}?", suggestions.join(", ")));
            }
            return Ok((false, 0, msg));
        }
    };
    let tgt_idx = match tgt_scored.first() {
        Some((_, idx)) => *idx,
        None => {
            let mut msg = format!("No matching node for '{}'.", target_query);
            let suggestions = nearest_labels(&loaded, target_query, SUGGESTION_COUNT);
            if !suggestions.is_empty() {
                msg.push_str(&format!(" Did you mean: {}?", suggestions.join(", ")));
            }
            return Ok((false, 0, msg));
        }
    };

    let path = match shortest_path_bfs(&loaded, src_idx, tgt_idx, directed) {
        Some(p) => p,
        None => return Ok((false, 0, "No path found.".to_string())),
    };

    let hops = path.len().saturating_sub(1);
    let mut text = format!("Shortest path ({} hops):\n", hops);

    for i in 0..path.len().saturating_sub(1) {
        let src = &loaded.graph[path[i]];
        let tgt = &loaded.graph[path[i + 1]];
        let edge_info = edge_between(&loaded.graph, path[i], path[i + 1]);
        let rel = edge_info.map_or("?".to_string(), |e| e.relation.clone());
        let conf = edge_info.map_or("?".to_string(), |e| e.confidence.clone());
        text.push_str(&format!(
            "  {} --{} [{}]--> {}\n",
            src.label, rel, conf, tgt.label
        ));
    }

    let answer = format!("path found: {} hops", hops);
    log_query(
        db,
        &format!("{} -> {}", source_query, target_query),
        &answer,
    );

    Ok((true, hops, text))
}

pub fn explain_with_neighbors(
    db: &Connection,
    db_path: &str,
    node_id: &str,
) -> graphify_core::Result<Option<ExplainResult>> {
    let loaded = load_graph_cached(db, db_path)?;

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
    // Explain is a lookup, not a traversal: neighbors in both directions.
    let mut seen: HashSet<NodeIndex> = HashSet::new();
    let mut neighbors: Vec<EdgeInfoResult> = Vec::new();
    for neighbor in iter_neighbors(&loaded.graph, idx, false) {
        if !seen.insert(neighbor) {
            continue;
        }
        let neighbor_data = &loaded.graph[neighbor];
        let edge = edge_between(&loaded.graph, idx, neighbor);
        neighbors.push(EdgeInfoResult {
            neighbor_id: neighbor_data.id.clone(),
            neighbor_label: neighbor_data.label.clone(),
            neighbor_file: loaded.display_path(&neighbor_data.source_file),
            relation: edge.map_or("?".to_string(), |e| e.relation.clone()),
            confidence: edge.map_or("?".to_string(), |e| e.confidence.clone()),
            strength: edge.map_or(0.0, |e| e.strength()),
        });
    }

    // Strongest connections first; ties broken deterministically.
    neighbors.sort_by(|a, b| {
        b.strength
            .partial_cmp(&a.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.relation.cmp(&b.relation))
            .then_with(|| a.neighbor_id.cmp(&b.neighbor_id))
    });
    let neighbor_count = neighbors.len();
    neighbors.truncate(20);

    let answer = format!("explain: {} ({} neighbors)", node.label, neighbor_count);
    log_query(db, node_id, &answer);

    Ok(Some(ExplainResult {
        id: node.id.clone(),
        label: node.label.clone(),
        source_file: loaded.display_path(&node.source_file),
        community: node.community,
        neighbor_count,
        neighbors,
    }))
}

pub struct EdgeInfoResult {
    pub neighbor_id: String,
    pub neighbor_label: String,
    pub neighbor_file: String,
    pub relation: String,
    pub confidence: String,
    pub strength: f64,
}

pub struct ExplainResult {
    pub id: String,
    pub label: String,
    pub source_file: String,
    pub community: Option<i64>,
    pub neighbor_count: usize,
    pub neighbors: Vec<EdgeInfoResult>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;

    fn seed(db: &Connection) -> String {
        db.execute_batch(
            "INSERT INTO nodes (id, label, file_type, source_file) VALUES
                ('a', 'Alpha', 'code', 'src/alpha.rs'),
                ('b', 'BetaService', 'code', 'src/beta.rs'),
                ('c', 'Gamma', 'code', 'src/gamma.rs'),
                ('d', 'Delta', 'code', 'src/delta.rs');
            INSERT INTO edges (source, target, relation, confidence, confidence_score, source_file) VALUES
                ('a', 'b', 'calls', 'EXTRACTED', 1.0, 'src/alpha.rs'),
                ('b', 'c', 'calls', 'SEMANTIC', NULL, 'src/beta.rs'),
                ('c', 'd', 'imports', 'INFERRED', 0.7, 'src/gamma.rs');",
        )
        .unwrap();
        "test".to_string()
    }

    fn loaded(db: &Connection, key: &str) -> Arc<LoadedGraph> {
        load_graph_cached(db, key).unwrap()
    }

    #[test]
    fn tokenize_splits_case_and_separators() {
        assert_eq!(
            tokenize("parseExtractionText"),
            vec!["parse", "extraction", "text"]
        );
        assert_eq!(
            tokenize("parse_extraction_text"),
            vec!["parse", "extraction", "text"]
        );
        assert_eq!(
            tokenize("Parse-Extraction.Text"),
            vec!["parse", "extraction", "text"]
        );
    }

    #[test]
    fn camel_query_finds_snake_label() {
        let db = open_db_in_memory().unwrap();
        db.execute_batch(
            "INSERT INTO nodes (id, label, file_type, source_file) VALUES
                ('n', 'parse_extraction_text', 'code', 'src/p.rs');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('n', 'n', 'calls', 'EXTRACTED', 'src/p.rs');",
        )
        .unwrap();
        let g = loaded(&db, "camel");
        let scored = score_nodes(&g, &["parseExtractionText".to_string()]);
        assert_eq!(
            scored.len(),
            1,
            "camelCase query should match snake_case label"
        );
    }

    #[test]
    fn directed_bfs_follows_edge_direction_only() {
        let db = open_db_in_memory().unwrap();
        let key = seed(&db);
        let g = loaded(&db, &key);

        let a = g.id_to_idx["a"];
        // b imports d? No — c imports d, so from 'a', directed BFS cannot
        // reach 'd' backwards through a->b<-? a->b->c->d is forward: use
        // 'd' as seed and confirm directed traversal does NOT walk
        // imports backwards to 'c'.
        let d = g.id_to_idx["d"];
        let (visited_fwd, _) = bfs_subgraph(&g, &[d], 2, true);
        assert!(!visited_fwd.contains(&g.id_to_idx["c"]));

        let (visited_und, _) = bfs_subgraph(&g, &[d], 2, false);
        assert!(visited_und.contains(&g.id_to_idx["c"]));

        let _ = a;
    }

    #[test]
    fn directed_path_respects_direction() {
        let db = open_db_in_memory().unwrap();
        let key = seed(&db);
        // a -> b -> c -> d: directed path a..d exists; d..a does not
        let (found, hops, _) = find_shortest_path(&db, &key, "Alpha", "Delta", true).unwrap();
        assert!(found);
        assert_eq!(hops, 3);

        let (found_rev, _, _) = find_shortest_path(&db, &key, "Delta", "Alpha", true).unwrap();
        assert!(!found_rev);

        let (found_und, hops_und, _) =
            find_shortest_path(&db, &key, "Delta", "Alpha", false).unwrap();
        assert!(found_und);
        assert_eq!(hops_und, 3);
    }

    #[test]
    fn explain_sorts_by_numeric_strength_not_alphabet() {
        let db = open_db_in_memory().unwrap();
        let key = seed(&db);
        let result = explain_with_neighbors(&db, &key, "b").unwrap().unwrap();
        assert_eq!(result.neighbors.len(), 2);
        // b -> c is SEMANTIC (fallback 0.6), a -> b is EXTRACTED 1.0.
        // Alphabetical ("EXTRACTED" < "SEMANTIC") would put SEMANTIC first.
        assert_eq!(result.neighbors[0].neighbor_id, "a");
        assert!(result.neighbors[0].strength > result.neighbors[1].strength);
    }

    #[test]
    fn scoring_is_deterministic_on_ties() {
        let db = open_db_in_memory().unwrap();
        db.execute_batch(
            "INSERT INTO nodes (id, label, file_type, source_file) VALUES
                ('x', 'handler', 'code', 'src/x.rs'),
                ('y', 'handler', 'code', 'src/y.rs');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES
                ('x', 'y', 'calls', 'EXTRACTED', 'src/x.rs');",
        )
        .unwrap();
        let g = loaded(&db, "ties");
        let s1 = score_nodes(&g, &["handler".to_string()]);
        let s2 = score_nodes(&g, &["handler".to_string()]);
        let ids = |s: &[(f64, NodeIndex)]| -> Vec<String> {
            s.iter().map(|(_, i)| g.graph[*i].id.clone()).collect()
        };
        assert_eq!(ids(&s1), ids(&s2));
    }

    #[test]
    fn confidence_rank_orders_labels() {
        assert!(confidence_rank("DECLARED") > confidence_rank("EXTRACTED"));
        assert!(confidence_rank("EXTRACTED") > confidence_rank("INFERRED"));
        assert!(confidence_rank("INFERRED") > confidence_rank("SEMANTIC"));
    }

    #[test]
    fn no_match_suggests_nearest_labels() {
        let db = open_db_in_memory().unwrap();
        let key = seed(&db);
        let (text, nodes, _) = query_graph(
            &db,
            &key,
            "Alpga completly-unrelated-xyzzy",
            "bfs",
            2,
            2000,
            false,
        )
        .unwrap();
        assert_eq!(nodes, 0);
        assert!(
            text.contains("Did you mean") && text.contains("Alpha"),
            "should suggest the near-miss label, got: {text}"
        );
    }

    #[test]
    fn node_lines_carry_ids_and_relative_paths() {
        let db = open_db_in_memory().unwrap();
        // Seed with an absolute source path under a fake root; the DB path
        // determines the root.
        db.execute_batch(
            "INSERT INTO nodes (id, label, file_type, source_file) VALUES
                ('a', 'Alpha', 'code', 'C:/repo/src/alpha.rs');
            INSERT INTO edges (source, target, relation, confidence, confidence_score, source_file) VALUES
                ('a', 'a', 'calls', 'EXTRACTED', 1.0, 'C:/repo/src/alpha.rs');",
        )
        .unwrap();
        let (text, _, _) = query_graph(
            &db,
            "C:/repo/.graphify/db.sqlite",
            "Alpha",
            "bfs",
            2,
            2000,
            false,
        )
        .unwrap();
        assert!(
            text.contains("[id=a src=src/alpha.rs"),
            "expected id and root-relative path in output, got: {text}"
        );
        assert!(!text.contains("C:/repo/src"), "absolute path must not leak");
    }

    #[test]
    fn tiny_budget_keeps_edges_not_just_nodes() {
        let db = open_db_in_memory().unwrap();
        db.execute_batch(
            "INSERT INTO nodes (id, label, file_type, source_file, docstring) VALUES
                ('a', 'Alpha', 'code', 'f.rs', 'docstring-a'),
                ('b', 'Beta', 'code', 'f.rs', 'docstring-b'),
                ('c', 'Gamma', 'code', 'f.rs', 'docstring-c'),
                ('d', 'Delta', 'code', 'f.rs', 'docstring-d');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES
                ('a', 'b', 'calls', 'EXTRACTED', 'f.rs'),
                ('a', 'c', 'calls', 'EXTRACTED', 'f.rs'),
                ('a', 'd', 'calls', 'EXTRACTED', 'f.rs');",
        )
        .unwrap();
        let (text, _, _) =
            query_graph(&db, ":memory:edgebudget", "Alpha", "bfs", 2, 1, false).unwrap();
        assert!(
            text.contains("EDGE"),
            "edge lines must survive a tiny budget, got: {text}"
        );
    }
}
