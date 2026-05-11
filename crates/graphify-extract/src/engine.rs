use std::path::{Path, PathBuf};

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser};

use crate::langs::{self, LanguageConfig};
use crate::schema::{ExtractedEdge, ExtractedNode, Extraction};
use graphify_core::GraphifyError;
use graphify_paths::normalize;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Join parts with `::` for hierarchical node IDs (e.g. "main.py::Greeter::greet()").
/// Preserves original casing. Sanitizes only path separators and whitespace.
fn make_node_id(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|p| p.trim().replace('\\', "/").replace('/', "_"))
        .collect::<Vec<_>>()
        .join("::")
}

/// Create a short target ID for cross-file references (imports, calls).
/// Normalizes to lowercase alphanumeric + underscores for fuzzy matching.
fn make_target_id(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_end_matches('_')
        .to_string()
}

/// Return `parent_dir_stem/file_stem` for uniqueness.
fn file_stem(path: &Path) -> String {
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if parent.is_empty() {
        file_name.to_string()
    } else {
        format!("{}_{}", parent, file_name)
    }
}

/// Extract UTF-8 text from a source byte slice for the given node.
fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    let range = node.byte_range();
    std::str::from_utf8(&source[range]).unwrap_or("")
}

/// Get the text of the first child of a node. Returns None if no children.
fn first_child_text<'a>(node: &Node<'a>, source: &'a [u8]) -> Option<&'a str> {
    let mut cursor = node.walk();
    let child = node.children(&mut cursor).next()?;
    Some(node_text(&child, source))
}

/// Get the second child of a tree-sitter node.
fn second_child<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let child = node.children(&mut cursor).nth(1);
    drop(cursor);
    child
}

/// SHA-256 hash of the file contents.
fn file_hash(path: &Path) -> Result<String, GraphifyError> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Check the extraction_cache table. Returns cached Extraction if hit.
fn check_cache(db: &Connection, path: &Path, hash: &str) -> Option<Extraction> {
    let path_str = normalize(path);
    let mut stmt = db
        .prepare(
            "SELECT language, nodes, edges FROM extraction_cache WHERE file_path = ?1 AND content_hash = ?2",
        )
        .ok()?;
    stmt.query_row(rusqlite::params![&path_str, hash], |row| {
        let language: String = row.get(0)?;
        let nodes_json: String = row.get(1)?;
        let edges_json: String = row.get(2)?;
        Ok((language, nodes_json, edges_json))
    })
    .ok()
    .map(|(language, nodes_json, edges_json)| {
        let nodes: Vec<ExtractedNode> = serde_json::from_str(&nodes_json).unwrap_or_default();
        let edges: Vec<ExtractedEdge> = serde_json::from_str(&edges_json).unwrap_or_default();
        Extraction {
            file_path: path.to_path_buf(),
            language,
            nodes,
            edges,
        }
    })
}

/// Save extraction result to the cache table.
fn save_cache(db: &Connection, path: &Path, hash: &str, extraction: &Extraction) {
    let path_str = normalize(path);
    let nodes_json = serde_json::to_string(&extraction.nodes).unwrap_or_default();
    let edges_json = serde_json::to_string(&extraction.edges).unwrap_or_default();
    let now = chrono_free_timestamp();

    if let Err(e) = db.execute(
        "INSERT OR REPLACE INTO extraction_cache (file_path, content_hash, language, nodes, edges, extracted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            &path_str,
            hash,
            extraction.language,
            nodes_json,
            edges_json,
            now,
        ],
    ) {
        eprintln!("warning: failed to cache extraction for {}: {}", path_str, e);
    }
}

/// Simple timestamp without needing chrono.
fn chrono_free_timestamp() -> String {
    format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    )
}

/// Find the body node using body_field first, then falling back to child types.
#[allow(clippy::manual_find)]
fn find_body<'a>(node: &Node<'a>, cfg: &LanguageConfig) -> Option<Node<'a>> {
    if let Some(field) = cfg.body_field {
        if let Some(body) = node.child_by_field_name(field) {
            return Some(body);
        }
    }
    // Fallback: look for a child whose kind is in body_fallback_types
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if cfg.body_fallback_types.contains(&child.kind()) {
            return Some(child);
        }
    }
    None
}

/// Extract the docstring: the first string/expression in the body.
fn extract_docstring(node: &Node, source: &[u8], cfg: &LanguageConfig) -> Option<String> {
    let body = find_body(node, cfg)?;
    let mut cursor = body.walk();
    if let Some(child) = body.children(&mut cursor).next() {
        let kind = child.kind();
        if kind == "string" || kind == "string_literal" || kind == "expression_statement" {
            let text = node_text(&child, source);
            // Strip quotes
            let cleaned = text
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim_start_matches("\"\"\"")
                .trim_end_matches("\"\"\"")
                .trim_start_matches("'''")
                .trim_end_matches("'''")
                .trim();
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pass 1: Structural extraction + inline call-graph
// ---------------------------------------------------------------------------

struct ExtractionState<'a> {
    cfg: &'a LanguageConfig,
    source: &'a [u8],
    file_id: String,
    file_path: PathBuf,
    nodes: Vec<ExtractedNode>,
    edges: Vec<ExtractedEdge>,
    current_class_id: Option<String>,
}

fn walk_structural<'a>(state: &mut ExtractionState<'a>, node: &Node<'a>) {
    let kind = node.kind();

    // --- Imports ---
    if state.cfg.import_types.contains(&kind) {
        // Call-name filter: if import_call_names is non-empty, check first child text
        let passes_filter = if state.cfg.import_call_names.is_empty() {
            true
        } else {
            first_child_text(node, state.source)
                .map(|t| state.cfg.import_call_names.contains(&t))
                .unwrap_or(false)
        };

        if passes_filter {
            let import_text = node_text(node, state.source);
            let module_name = extract_import_module(import_text, kind, state.cfg.name);
            if let Some(mod_name) = module_name {
                let target_id = make_target_id(&mod_name);
                state.edges.push(ExtractedEdge {
                    source: state.file_id.clone(),
                    target: target_id,
                    relation: "imports".to_string(),
                    confidence: "EXTRACTED".to_string(),
                    confidence_score: Some(1.0),
                    source_file: state.file_path.clone(),
                    source_line: Some(node.start_position().row as u32),
                });
            }
        }
        // Still walk children for nested structures
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_structural(state, &child);
        }
        return;
    }

    // --- Classes / structs / enums ---
    if state.cfg.class_types.contains(&kind) {
        // Call-name filter: if class_call_names is non-empty, check first child text
        let passes_filter = if state.cfg.class_call_names.is_empty() {
            true
        } else {
            first_child_text(node, state.source)
                .map(|t| state.cfg.class_call_names.contains(&t))
                .unwrap_or(false)
        };

        if passes_filter {
            // Try name_field first, then fall back to second child for call-based languages
            let name_node = node.child_by_field_name(state.cfg.name_field);
            let name_node = match name_node {
                Some(n) => Some(n),
                None if !state.cfg.class_call_names.is_empty() => second_child(node),
                _ => None,
            };
            if let Some(name_node) = name_node {
                let name = node_text(&name_node, state.source).to_string();
                let class_id = make_node_id(&[&state.file_id, &name]);
                let docstring = extract_docstring(node, state.source, state.cfg);

                state.nodes.push(ExtractedNode {
                    id: class_id.clone(),
                    label: name.clone(),
                    source_file: state.file_path.clone(),
                    source_line: Some(node.start_position().row as u32),
                    docstring,
                    node_type: "class".to_string(),
                });

                state.edges.push(ExtractedEdge {
                    source: state.file_id.clone(),
                    target: class_id.clone(),
                    relation: "contains".to_string(),
                    confidence: "EXTRACTED".to_string(),
                    confidence_score: Some(1.0),
                    source_file: state.file_path.clone(),
                    source_line: Some(node.start_position().row as u32),
                });

                // Walk children inside this class context
                let prev_class = state.current_class_id.replace(class_id);
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    walk_structural(state, &child);
                }
                state.current_class_id = prev_class;
                return;
            }
        }
    }

    // --- Functions / methods ---
    if state.cfg.function_types.contains(&kind) {
        // Call-name filter: if function_call_names is non-empty, check first child text
        let passes_filter = if state.cfg.function_call_names.is_empty() {
            true
        } else {
            first_child_text(node, state.source)
                .map(|t| state.cfg.function_call_names.contains(&t))
                .unwrap_or(false)
        };

        if passes_filter {
            // Try name_field first, then fall back to second child for call-based languages
            let name_node = node.child_by_field_name(state.cfg.name_field);
            let name_node = match name_node {
                Some(n) => Some(n),
                None if !state.cfg.function_call_names.is_empty() => second_child(node),
                _ => None,
            };
            if let Some(name_node) = name_node {
                let name = node_text(&name_node, state.source).to_string();
                let func_label = format!("{}()", name);
                let parent_id = state.current_class_id.as_deref().unwrap_or(&state.file_id);
                let func_id = make_node_id(&[parent_id, &name]);

                let docstring = extract_docstring(node, state.source, state.cfg);

                state.nodes.push(ExtractedNode {
                    id: func_id.clone(),
                    label: func_label,
                    source_file: state.file_path.clone(),
                    source_line: Some(node.start_position().row as u32),
                    docstring,
                    node_type: "function".to_string(),
                });

                state.edges.push(ExtractedEdge {
                    source: parent_id.to_string(),
                    target: func_id.clone(),
                    relation: "contains".to_string(),
                    confidence: "EXTRACTED".to_string(),
                    confidence_score: Some(1.0),
                    source_file: state.file_path.clone(),
                    source_line: Some(node.start_position().row as u32),
                });

                // Pass 2 inline: walk function body for call expressions
                if let Some(body) = find_body(node, state.cfg) {
                    walk_calls(state, &func_id, &body);
                }
            }
        }

        // Walk children for nested functions
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_structural(state, &child);
        }
        return;
    }

    // Default: recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_structural(state, &child);
    }
}

/// Best-effort module name extraction from import text.
/// Dispatches on language first to avoid conflicts between languages
/// that share the same tree-sitter node kinds (e.g. "import_statement"
/// is used by both Python and JavaScript).
fn extract_import_module(text: &str, kind: &str, language: &str) -> Option<String> {
    match (language, kind) {
        ("Python", "import_statement" | "import_from_statement") => {
            let cleaned = text
                .trim()
                .trim_start_matches("import ")
                .trim_start_matches("from ");
            let first = cleaned.split_whitespace().next()?;
            let module = first.split('.').next()?;
            Some(module.to_string())
        }
        ("JavaScript" | "TypeScript", "import_statement" | "import_declaration") => {
            if let Some(pos) = text.find("from") {
                let after_from = &text[pos + 4..];
                let trimmed = after_from.trim();
                let module = trimmed
                    .trim_start_matches('"')
                    .trim_start_matches('\'')
                    .trim_start_matches('`')
                    .split(&['"', '\'', '`'][..])
                    .next()
                    .unwrap_or("");
                if !module.is_empty() {
                    return Some(module.to_string());
                }
            }
            // Require-style: require('module')
            if let Some(pos) = text.find("require(") {
                let after = &text[pos + 8..];
                let module = after
                    .trim_start_matches('"')
                    .trim_start_matches('\'')
                    .split(&['"', '\'', ')'][..])
                    .next()
                    .unwrap_or("");
                if !module.is_empty() {
                    return Some(module.to_string());
                }
            }
            None
        }
        ("Rust", "use_declaration") => {
            let cleaned = text.trim_start_matches("use").trim().trim_end_matches(';');
            let first = cleaned.split("::").next()?.trim();
            if !first.is_empty() {
                return Some(first.to_string());
            }
            None
        }
        ("Go", "import_declaration") => {
            let cleaned = text.trim_start_matches("import").trim();
            let module = cleaned
                .trim_start_matches('"')
                .split(&['"', '\n'][..])
                .next()
                .unwrap_or("");
            if !module.is_empty() {
                return Some(module.to_string());
            }
            None
        }
        ("Java" | "Scala", "import_declaration") => {
            let cleaned = text
                .trim_start_matches("import")
                .trim_start_matches("static")
                .trim()
                .trim_end_matches(';');
            let parts: Vec<&str> = cleaned.split('.').collect();
            if !parts.is_empty() {
                return Some(parts.join("."));
            }
            None
        }
        ("Swift", "import_declaration") => {
            let cleaned = text.trim_start_matches("import").trim();
            let module = cleaned.split_whitespace().next()?;
            if !module.is_empty() {
                return Some(module.to_string());
            }
            None
        }
        ("C" | "C++", "preproc_include") => {
            let cleaned = text.trim_start_matches("#include").trim();
            let module = cleaned
                .trim_start_matches('<')
                .trim_start_matches('"')
                .split(&['>', '"'][..])
                .next()
                .unwrap_or("");
            if !module.is_empty() {
                return Some(module.to_string());
            }
            None
        }
        ("CSS", "import_statement") => {
            let cleaned = text.trim_start_matches("@import").trim();
            let module = cleaned
                .trim_start_matches('"')
                .trim_start_matches('\'')
                .trim_start_matches("url(")
                .trim_start_matches('"')
                .trim_start_matches('\'')
                .split(&['"', '\'', ')'][..])
                .next()
                .unwrap_or("");
            if !module.is_empty() {
                return Some(module.to_string());
            }
            None
        }
        ("Elixir", "call") => {
            // Elixir imports: use MyModule, import MyModule, alias My.Module, require MyModule
            let cleaned = text.trim();
            let keyword = cleaned.split_whitespace().next()?;
            if !matches!(keyword, "use" | "import" | "alias" | "require") {
                return None;
            }
            let after_keyword = cleaned.trim_start_matches(keyword).trim();
            let module = after_keyword.split(&[' ', ',', '.'][..]).next()?;
            if !module.is_empty() {
                return Some(module.to_string());
            }
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Pass 2: Call-graph extraction (walked inline during pass 1)
// ---------------------------------------------------------------------------

fn walk_calls<'a>(state: &mut ExtractionState<'a>, caller_id: &str, body: &Node<'a>) {
    let kind = body.kind();

    if kind == state.cfg.call_type {
        let callee_name = extract_callee_name(body, state.source);
        if let Some(name) = callee_name {
            let callee_id = make_target_id(&name);
            state.edges.push(ExtractedEdge {
                source: caller_id.to_string(),
                target: callee_id,
                relation: "calls".to_string(),
                confidence: "INFERRED".to_string(),
                confidence_score: Some(0.7),
                source_file: state.file_path.clone(),
                source_line: Some(body.start_position().row as u32),
            });
        }
    }

    // Recurse into children
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        walk_calls(state, caller_id, &child);
    }
}

/// Extract the callee name from a call expression.
fn extract_callee_name(call_node: &Node, source: &[u8]) -> Option<String> {
    // The first child (field "function") is the callee
    let mut cursor = call_node.walk();
    let func_child = call_node.children(&mut cursor).next()?;

    let text = node_text(&func_child, source);
    // For method calls like obj.method(), take the last part
    let name = if text.contains('.') {
        text.split('.').next_back().unwrap_or(text)
    } else {
        text
    };
    Some(name.to_string())
}

// ---------------------------------------------------------------------------
// Rationale comment extraction
// ---------------------------------------------------------------------------

const RATIONALE_TAGS: &[&str] = &["NOTE", "WHY", "HACK", "IMPORTANT", "TODO", "FIXME"];

fn extract_rationale(state: &mut ExtractionState, source: &[u8]) {
    let comment_prefix = if state.cfg.name == "Python" {
        "#"
    } else {
        "//"
    };
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return,
    };

    // Build a sorted list of (line, node_id) to find nearest parent above each rationale
    let mut nodes_by_line: Vec<(u32, String)> = state
        .nodes
        .iter()
        .filter_map(|n| n.source_line.map(|l| (l, n.id.clone())))
        .collect();
    nodes_by_line.sort_by_key(|(l, _)| *l);

    for (lineno, line_text) in text.lines().enumerate() {
        let stripped = line_text.trim();
        let tag = match find_rationale_tag(stripped, comment_prefix) {
            Some(t) => t,
            None => continue,
        };

        let comment_text = stripped
            .trim_start_matches(comment_prefix)
            .trim_start_matches(&format!("{}:", tag))
            .trim();

        if comment_text.is_empty() {
            continue;
        }

        let line_num = lineno as u32 + 1;
        let rid = make_node_id(&[&state.file_id, "rationale", &line_num.to_string()]);

        // Find nearest parent node above this line
        let parent_id = nodes_by_line
            .iter()
            .rev()
            .find(|(l, _)| *l < line_num)
            .map(|(_, id)| id.clone())
            .unwrap_or_else(|| state.file_id.clone());

        let label = if comment_text.len() > 80 {
            format!("{}: {}", tag, &comment_text[..80])
        } else {
            format!("{}: {}", tag, comment_text)
        };

        state.nodes.push(ExtractedNode {
            id: rid.clone(),
            label,
            source_file: state.file_path.clone(),
            source_line: Some(line_num),
            docstring: None,
            node_type: "rationale".to_string(),
        });

        state.edges.push(ExtractedEdge {
            source: rid,
            target: parent_id,
            relation: "rationale_for".to_string(),
            confidence: "EXTRACTED".to_string(),
            confidence_score: Some(1.0),
            source_file: state.file_path.clone(),
            source_line: Some(line_num),
        });
    }
}

fn find_rationale_tag(line: &str, comment_prefix: &str) -> Option<&'static str> {
    for tag in RATIONALE_TAGS {
        let pattern = format!("{} {}:", comment_prefix, tag);
        if line.starts_with(&pattern) {
            return Some(tag);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Single-file extraction
// ---------------------------------------------------------------------------

fn extract_single(path: &Path, cfg: &LanguageConfig) -> Result<Extraction, GraphifyError> {
    let source = std::fs::read(path)?;
    let source_ref = source.as_slice();

    let language = (cfg.language_fn)();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| GraphifyError::Parse {
            file: path.display().to_string(),
            message: e.to_string(),
        })?;

    let tree = parser
        .parse(source_ref, None)
        .ok_or_else(|| GraphifyError::Parse {
            file: path.display().to_string(),
            message: "parse returned None".to_string(),
        })?;

    let root = tree.root_node();
    let fid = file_stem(path);
    let file_id = make_node_id(&[&fid]);

    let mut state = ExtractionState {
        cfg,
        source: &source,
        file_id,
        file_path: path.to_path_buf(),
        nodes: Vec::new(),
        edges: Vec::new(),
        current_class_id: None,
    };

    // Add file node
    state.nodes.push(ExtractedNode {
        id: state.file_id.clone(),
        label: path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string(),
        source_file: path.to_path_buf(),
        source_line: None,
        docstring: None,
        node_type: "file".to_string(),
    });

    // Walk the AST (pass 1 structural + inline pass 2 call graph)
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        walk_structural(&mut state, &child);
    }

    // Post-pass: extract rationale comments
    extract_rationale(&mut state, &source);

    Ok(Extraction {
        file_path: path.to_path_buf(),
        language: cfg.name.to_string(),
        nodes: state.nodes,
        edges: state.edges,
    })
}

// ---------------------------------------------------------------------------
// Markdown extraction (plain-text, no tree-sitter)
// ---------------------------------------------------------------------------

fn extract_markdown(path: &Path) -> Result<Extraction, GraphifyError> {
    let bytes = std::fs::read(path)?;
    let content = String::from_utf8_lossy(&bytes).into_owned();
    Ok(extract_markdown_from_string(path, "markdown", &content))
}

/// Extract markdown-style structure from a string. Used for both .md/.mdx files
/// and PDF files (converted to markdown).
fn extract_markdown_from_string(path: &Path, language: &str, content: &str) -> Extraction {
    let fid = file_stem(path);
    let file_id = make_node_id(&[&fid]);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Document node
    nodes.push(ExtractedNode {
        id: file_id.clone(),
        label: path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string(),
        source_file: path.to_path_buf(),
        source_line: None,
        docstring: None,
        node_type: "document".to_string(),
    });

    let heading_re = regex::Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
    let link_re = regex::Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap();

    // Track heading nesting: stack of (level, id)
    let mut heading_stack: Vec<(usize, String)> = Vec::new();

    for (line_no, line) in content.lines().enumerate() {
        // Parse headings
        if let Some(caps) = heading_re.captures(line) {
            let hashes = caps.get(1).unwrap().as_str().len();
            let title = caps.get(2).unwrap().as_str().trim().to_string();
            let level = hashes;
            let slug = make_target_id(&title);
            let section_id = make_node_id(&[&fid, &slug]);

            nodes.push(ExtractedNode {
                id: section_id.clone(),
                label: title,
                source_file: path.to_path_buf(),
                source_line: Some(line_no as u32 + 1),
                docstring: None,
                node_type: "section".to_string(),
            });

            // Pop stack until we find a parent with lower level
            while let Some((parent_level, _)) = heading_stack.last() {
                if *parent_level < level {
                    break;
                }
                heading_stack.pop();
            }

            // Edge: parent heading → this heading, or file → this heading
            let parent_id = heading_stack
                .last()
                .map(|(_, id)| id.clone())
                .unwrap_or_else(|| file_id.clone());

            edges.push(ExtractedEdge {
                source: parent_id,
                target: section_id.clone(),
                relation: "contains".to_string(),
                confidence: "EXTRACTED".to_string(),
                confidence_score: Some(1.0),
                source_file: path.to_path_buf(),
                source_line: Some(line_no as u32 + 1),
            });

            heading_stack.push((level, section_id));
        }

        // Parse links (only local .md references)
        for cap in link_re.captures_iter(line) {
            let link_target = cap.get(2).unwrap().as_str();
            // Only reference local markdown files
            if link_target.starts_with("http") || link_target.starts_with('#') {
                continue;
            }
            let target_path = if link_target.starts_with('/') {
                link_target.to_string()
            } else {
                // Relative path — just use the file stem as target
                link_target.to_string()
            };
            let target_stem = std::path::Path::new(&target_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if !target_stem.is_empty() {
                let target_id = make_target_id(target_stem);
                edges.push(ExtractedEdge {
                    source: file_id.clone(),
                    target: target_id,
                    relation: "references".to_string(),
                    confidence: "EXTRACTED".to_string(),
                    confidence_score: Some(0.8),
                    source_file: path.to_path_buf(),
                    source_line: Some(line_no as u32 + 1),
                });
            }
        }
    }

    Extraction {
        file_path: path.to_path_buf(),
        language: language.to_string(),
        nodes,
        edges,
    }
}

/// Extract structure from plain text files (.txt).
fn extract_text_file(path: &Path, language: &str) -> Result<Extraction, GraphifyError> {
    let bytes = std::fs::read(path)?;
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let fid = file_stem(path);
    let file_id = make_node_id(&[&fid]);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    nodes.push(ExtractedNode {
        id: file_id.clone(),
        label: path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string(),
        source_file: path.to_path_buf(),
        source_line: None,
        docstring: None,
        node_type: "document".to_string(),
    });

    let mut line_no = 0u32;
    let mut para_index = 0u32;
    let mut current_para_start: Option<(u32, String)> = None;

    for line in content.lines() {
        line_no += 1;
        if line.trim().is_empty() {
            if let Some((start_line, text)) = current_para_start.take() {
                let label = if text.len() > 80 {
                    format!("{}...", &text[..77])
                } else {
                    text
                };
                let section_id = make_node_id(&[&fid, &format!("p{}", para_index)]);

                nodes.push(ExtractedNode {
                    id: section_id.clone(),
                    label,
                    source_file: path.to_path_buf(),
                    source_line: Some(start_line),
                    docstring: None,
                    node_type: "section".to_string(),
                });
                edges.push(ExtractedEdge {
                    source: file_id.clone(),
                    target: section_id,
                    relation: "contains".to_string(),
                    confidence: "EXTRACTED".to_string(),
                    confidence_score: Some(1.0),
                    source_file: path.to_path_buf(),
                    source_line: Some(start_line),
                });
                para_index += 1;
            }
        } else {
            if current_para_start.is_none() {
                current_para_start = Some((line_no, line.trim().to_string()));
            }
        }
    }
    // Handle trailing paragraph
    if let Some((start_line, text)) = current_para_start.take() {
        let label = if text.len() > 80 {
            format!("{}...", &text[..77])
        } else {
            text
        };
        let section_id = make_node_id(&[&fid, &format!("p{}", para_index)]);
        nodes.push(ExtractedNode {
            id: section_id.clone(),
            label,
            source_file: path.to_path_buf(),
            source_line: Some(start_line),
            docstring: None,
            node_type: "section".to_string(),
        });
        edges.push(ExtractedEdge {
            source: file_id.clone(),
            target: section_id,
            relation: "contains".to_string(),
            confidence: "EXTRACTED".to_string(),
            confidence_score: Some(1.0),
            source_file: path.to_path_buf(),
            source_line: Some(start_line),
        });
    }

    Ok(Extraction {
        file_path: path.to_path_buf(),
        language: language.to_string(),
        nodes,
        edges,
    })
}

/// Extract structure from reStructuredText files (.rst).
fn extract_rst(path: &Path) -> Result<Extraction, GraphifyError> {
    let bytes = std::fs::read(path)?;
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let fid = file_stem(path);
    let file_id = make_node_id(&[&fid]);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    nodes.push(ExtractedNode {
        id: file_id.clone(),
        label: path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string(),
        source_file: path.to_path_buf(),
        source_line: None,
        docstring: None,
        node_type: "document".to_string(),
    });

    let lines: Vec<&str> = content.lines().collect();
    let heading_chars: &[char] = &['=', '-', '~', '^', '"'];

    // Track heading nesting: stack of (underline_char, id)
    let mut heading_stack: Vec<(char, String)> = Vec::new();

    let mut i = 0;
    while i + 1 < lines.len() {
        let text_line = lines[i];
        let under_line = lines[i + 1];

        let trimmed_text = text_line.trim();
        let is_heading = !trimmed_text.is_empty()
            && !under_line.trim().is_empty()
            && under_line
                .trim()
                .chars()
                .all(|c| heading_chars.contains(&c))
            && under_line.trim().len() >= trimmed_text.len();

        if is_heading {
            let ch = under_line.trim().chars().next().unwrap_or('=');
            let title = trimmed_text.to_string();
            let slug = make_target_id(&title);
            let section_id = make_node_id(&[&fid, &slug]);

            nodes.push(ExtractedNode {
                id: section_id.clone(),
                label: title,
                source_file: path.to_path_buf(),
                source_line: Some(i as u32 + 1),
                docstring: None,
                node_type: "section".to_string(),
            });

            // Pop stack until we find a parent with a different (higher-rank) char
            let char_rank = |c: char| heading_chars.iter().position(|&h| h == c).unwrap_or(0);
            while let Some((parent_ch, _)) = heading_stack.last() {
                if char_rank(*parent_ch) < char_rank(ch) {
                    break;
                }
                heading_stack.pop();
            }

            let parent_id = heading_stack
                .last()
                .map(|(_, id)| id.clone())
                .unwrap_or_else(|| file_id.clone());

            edges.push(ExtractedEdge {
                source: parent_id,
                target: section_id.clone(),
                relation: "contains".to_string(),
                confidence: "EXTRACTED".to_string(),
                confidence_score: Some(1.0),
                source_file: path.to_path_buf(),
                source_line: Some(i as u32 + 1),
            });

            heading_stack.push((ch, section_id));
            i += 2;
            continue;
        }
        i += 1;
    }

    Ok(Extraction {
        file_path: path.to_path_buf(),
        language: "rst".to_string(),
        nodes,
        edges,
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn extract(files: &[PathBuf], db: &Connection) -> Result<Vec<Extraction>, GraphifyError> {
    let mut results = Vec::new();

    for file_path in files {
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let hash = file_hash(file_path)?;

        // Markdown: plain-text extraction (no tree-sitter)
        if ext == "md" || ext == "mdx" {
            if let Some(cached) = check_cache(db, file_path, &hash) {
                results.push(cached);
                continue;
            }
            let extraction = extract_markdown(file_path)?;
            save_cache(db, file_path, &hash, &extraction);
            results.push(extraction);
            continue;
        }

        // PDF: extract text via graphify-pdf, then parse as markdown
        if ext == "pdf" {
            if let Some(cached) = check_cache(db, file_path, &hash) {
                results.push(cached);
                continue;
            }
            match graphify_pdf::extract_to_markdown(file_path) {
                Ok(md_text) if !md_text.trim().is_empty() => {
                    let extraction = extract_markdown_from_string(file_path, "pdf", &md_text);
                    save_cache(db, file_path, &hash, &extraction);
                    results.push(extraction);
                }
                _ => {}
            }
            continue;
        }

        // Plain text: paragraph-based extraction
        if ext == "txt" {
            if let Some(cached) = check_cache(db, file_path, &hash) {
                results.push(cached);
                continue;
            }
            if let Ok(extraction) = extract_text_file(file_path, "text") {
                save_cache(db, file_path, &hash, &extraction);
                results.push(extraction);
            }
            continue;
        }

        // reStructuredText: heading-based extraction
        if ext == "rst" {
            if let Some(cached) = check_cache(db, file_path, &hash) {
                results.push(cached);
                continue;
            }
            if let Ok(extraction) = extract_rst(file_path) {
                save_cache(db, file_path, &hash, &extraction);
                results.push(extraction);
            }
            continue;
        }

        let cfg = match langs::get_language_for_extension(ext) {
            Some(c) => c,
            None => continue,
        };

        // Check cache
        if let Some(cached) = check_cache(db, file_path, &hash) {
            results.push(cached);
            continue;
        }

        // Extract
        let extraction = extract_single(file_path, cfg)?;

        // Save to cache
        save_cache(db, file_path, &hash, &extraction);

        results.push(extraction);
    }

    // Cross-file resolution: try to match call/import targets to known node IDs
    resolve_cross_file_references(&mut results);

    Ok(results)
}

/// Build a lookup of all known node IDs (lowercased for matching) and try to
/// resolve INFERRED call edges to real node IDs. This turns stub references
/// into proper cross-file edges when a match is found.
fn resolve_cross_file_references(results: &mut [Extraction]) {
    // Collect all known node IDs and their labels
    let mut known_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for ext in results.iter() {
        for node in &ext.nodes {
            // Map: lowercase label -> actual node ID
            known_ids.insert(node.label.to_lowercase(), node.id.clone());
            // Also map by the last segment of the ID (e.g. "greet" from "main::Greeter::greet")
            let parts: Vec<&str> = node.id.split("::").collect();
            if let Some(last) = parts.last() {
                let lower = last.to_lowercase().trim_end_matches("()").to_string();
                known_ids.entry(lower).or_insert_with(|| node.id.clone());
            }
        }
    }

    // Resolve edges
    for ext in results.iter_mut() {
        for edge in ext.edges.iter_mut() {
            if edge.relation == "calls" || edge.relation == "imports" {
                let target_lower = edge.target.to_lowercase();
                if let Some(real_id) = known_ids.get(&target_lower) {
                    if real_id != &edge.target {
                        edge.target = real_id.clone();
                        if edge.confidence == "INFERRED" {
                            edge.confidence = "EXTRACTED".to_string();
                            edge.confidence_score = Some(0.9);
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;
    use std::fs;

    #[test]
    fn extract_python_file() {
        let dir = tempfile::tempdir().unwrap();
        let py = dir.path().join("main.py");
        fs::write(
            &py,
            "\nclass Greeter:\n    \"\"\"Says hello\"\"\"\n    def greet(self, name):\n        print(name)\n\ndef helper():\n    pass\n",
        )
        .unwrap();
        let db = open_db_in_memory().unwrap();
        let results = extract(&[py], &db).unwrap();
        assert_eq!(results.len(), 1);
        let ext = &results[0];
        assert_eq!(ext.language, "Python");
        assert!(
            ext.nodes.iter().any(|n| n.label == "Greeter"),
            "missing class"
        );
        assert!(
            ext.nodes.iter().any(|n| n.label == "greet()"),
            "missing method"
        );
        assert!(
            ext.nodes.iter().any(|n| n.label == "helper()"),
            "missing function"
        );
        assert!(ext.edges.iter().any(|e| e.relation == "contains"));
    }

    #[test]
    fn extract_rust_file() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("main.rs");
        fs::write(
            &rs,
            "\nstruct Config {\n    name: String,\n}\n\nfn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        let db = open_db_in_memory().unwrap();
        let results = extract(&[rs], &db).unwrap();
        let ext = &results[0];
        assert_eq!(ext.language, "Rust");
        assert!(
            ext.nodes.iter().any(|n| n.label == "Config"),
            "missing struct"
        );
        assert!(
            ext.nodes.iter().any(|n| n.label == "main()"),
            "missing fn main"
        );
    }

    #[test]
    fn extract_javascript_file() {
        let dir = tempfile::tempdir().unwrap();
        let js = dir.path().join("app.js");
        fs::write(
            &js,
            "\nclass App {\n    start() {\n        console.log(\"hello\");\n    }\n}\nfunction helper() {\n    return 42;\n}\n",
        )
        .unwrap();
        let db = open_db_in_memory().unwrap();
        let results = extract(&[js], &db).unwrap();
        let ext = &results[0];
        assert_eq!(ext.language, "JavaScript");
        assert!(ext.nodes.iter().any(|n| n.label == "App"));
        assert!(ext.nodes.iter().any(|n| n.label == "helper()"));
    }

    #[test]
    fn extraction_uses_cache() {
        let dir = tempfile::tempdir().unwrap();
        let py = dir.path().join("main.py");
        fs::write(&py, "def hello(): pass\n").unwrap();
        let db = open_db_in_memory().unwrap();
        let r1 = extract(&[py.clone()], &db).unwrap();
        let r2 = extract(&[py], &db).unwrap();
        assert_eq!(r1[0].nodes.len(), r2[0].nodes.len());
    }

    #[test]
    fn extract_markdown_file() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("guide.md");
        fs::write(
            &md,
            "# Getting Started\n\nIntro text.\n\n## Installation\n\nSee [setup guide](setup.md) for details.\n\n### Step 1\n\nDo the thing.\n\n## Usage\n\nHow to use it.\n",
        ).unwrap();
        let db = open_db_in_memory().unwrap();
        let results = extract(&[md], &db).unwrap();
        let ext = &results[0];
        assert_eq!(ext.language, "markdown");

        // Document node + 4 section headings (Getting Started, Installation, Step 1, Usage)
        assert!(
            ext.nodes.len() >= 5,
            "expected >= 5 nodes, got {}",
            ext.nodes.len()
        );
        assert!(
            ext.nodes.iter().any(|n| n.node_type == "document"),
            "missing document node"
        );
        assert!(
            ext.nodes.iter().any(|n| n.label == "Getting Started"),
            "missing h1"
        );
        assert!(
            ext.nodes.iter().any(|n| n.label == "Installation"),
            "missing h2"
        );
        assert!(ext.nodes.iter().any(|n| n.label == "Step 1"), "missing h3");
        assert!(ext.nodes.iter().any(|n| n.label == "Usage"), "missing h2");

        // contains edges (doc → headings, parent → child)
        let contains: Vec<_> = ext
            .edges
            .iter()
            .filter(|e| e.relation == "contains")
            .collect();
        assert!(
            contains.len() >= 4,
            "expected >= 4 contains edges, got {}",
            contains.len()
        );

        // references edge to setup.md
        assert!(
            ext.edges
                .iter()
                .any(|e| e.relation == "references" && e.target.contains("setup")),
            "missing references edge to setup.md"
        );
    }

    #[test]
    fn extract_rationale_comments() {
        let dir = tempfile::tempdir().unwrap();
        let py = dir.path().join("main.py");
        fs::write(
            &py,
            "\ndef process(data):\n    # WHY: We need to normalize because upstream sends raw bytes\n    result = normalize(data)\n    # HACK: Temporary workaround for API bug\n    return result\n\nclass Handler:\n    # NOTE: This is not thread-safe\n    def handle(self):\n        pass\n",
        ).unwrap();
        let db = open_db_in_memory().unwrap();
        let results = extract(&[py], &db).unwrap();
        let ext = &results[0];

        let rationale_nodes: Vec<_> = ext
            .nodes
            .iter()
            .filter(|n| n.node_type == "rationale")
            .collect();
        assert!(
            rationale_nodes.len() >= 3,
            "expected >= 3 rationale nodes, got {}",
            rationale_nodes.len()
        );

        assert!(
            rationale_nodes.iter().any(|n| n.label.contains("WHY")),
            "missing WHY rationale"
        );
        assert!(
            rationale_nodes.iter().any(|n| n.label.contains("HACK")),
            "missing HACK rationale"
        );
        assert!(
            rationale_nodes.iter().any(|n| n.label.contains("NOTE")),
            "missing NOTE rationale"
        );

        let rationale_edges: Vec<_> = ext
            .edges
            .iter()
            .filter(|e| e.relation == "rationale_for")
            .collect();
        assert!(
            rationale_edges.len() >= 3,
            "expected >= 3 rationale_for edges, got {}",
            rationale_edges.len()
        );
    }
}
