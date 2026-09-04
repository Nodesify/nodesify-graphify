// walkers: tree-sitter AST extraction — structural pass (imports, classes,
// functions), inline call-graph pass, rationale comments, and the
// single-file driver that ties them together.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tree_sitter::{Node, Parser};

use crate::builtins::is_language_builtin;
use crate::langs::LanguageConfig;
use crate::naming::{file_stem, make_node_id, make_target_id};
use crate::schema::{ExtractedEdge, ExtractedNode, Extraction};
use graphify_core::GraphifyError;

// ---------------------------------------------------------------------------
// AST text helpers
// ---------------------------------------------------------------------------

/// Extract UTF-8 text from a source byte slice for the given node.
pub(crate) fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
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

/// Signature line(s): source text from the item start to the start of its
/// body, whitespace-collapsed and capped. Lets agents see WHAT a symbol is
/// without opening the file.
fn node_signature(node: &Node, source: &[u8], cfg: &LanguageConfig) -> Option<String> {
    let body_start = find_body(node, cfg)
        .map(|b| b.byte_range().start)
        .unwrap_or_else(|| node.byte_range().end);
    let start = node.byte_range().start;
    if body_start <= start {
        return None;
    }
    let raw = std::str::from_utf8(&source[start..body_start.min(source.len())]).unwrap_or("");
    let mut sig = String::new();
    for word in raw.split_whitespace() {
        sig.push_str(word);
        sig.push(' ');
        if sig.len() > 200 {
            break;
        }
    }
    let sig = sig.trim_end().to_string();
    if sig.is_empty() {
        None
    } else {
        Some(sig)
    }
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

pub(crate) struct ExtractionState<'a> {
    pub cfg: &'a LanguageConfig,
    pub source: &'a [u8],
    pub file_id: String,
    pub file_path: PathBuf,
    pub nodes: Vec<ExtractedNode>,
    pub edges: Vec<ExtractedEdge>,
    pub current_class_id: Option<String>,
    /// Identifier-shaped string literals already indexed for this file,
    /// keyed by lowercased literal (one reference node per distinct literal
    /// per file; the node itself is global across files).
    pub string_refs_seen: HashSet<String>,
}

/// Maximum reference nodes extracted from one file — bounds the graph cost
/// of string-literal indexing.
const MAX_STRING_REFS_PER_FILE: usize = 40;

/// True when a string literal is identifier-shaped enough to index as a
/// reference: env-var style (`PLANE_URL`, `NODE_ENV`), snake_case keys
/// (`needs_human`), and dotted/kebab/slash chains (`cli.command`,
/// `harness/hr-101-fix-redis-leak`). Plain words ("retry", "error") are
/// deliberately excluded — they are prose/UI text far more often than
/// shared references, and would drown the graph in noise.
fn is_reference_literal(s: &str) -> bool {
    if s.is_empty() || s.len() < 3 || s.len() > 64 {
        return false;
    }
    let bytes = s.as_bytes();
    if bytes.iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'_')
        && s.contains('_')
    {
        return true;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut separators = 0;
    for &b in bytes {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => {}
            b'_' | b'.' | b'-' | b'/' => separators += 1,
            _ => return false,
        }
    }
    separators >= 1
}

/// Strip a string literal's raw source text down to its content: remove
/// surrounding quotes/backticks and language prefixes (`r#"..."`, `b".."`).
fn unquote_literal(raw: &str) -> &str {
    let t = raw.trim();
    let body = if t.len() > 2 && t.as_bytes()[0].is_ascii_alphabetic() {
        &t[1..]
    } else {
        t
    };
    let bytes = body.as_bytes();
    let quoted = bytes.len() >= 2
        && matches!(
            (bytes.first().copied(), bytes.last().copied()),
            (Some(b'"'), Some(b'"'))
                | (Some(b'\''), Some(b'\''))
                | (Some(b'`'), Some(b'`'))
        );
    if quoted {
        &body[1..body.len() - 1]
    } else {
        body
    }
}

/// Collect an identifier-shaped string literal as a global reference node
/// (`str::<literal>` id) plus a `references` edge from this file's node.
fn collect_string_refs(state: &mut ExtractionState<'_>, node: &Node<'_>) {
    if state.string_refs_seen.len() >= MAX_STRING_REFS_PER_FILE {
        return;
    }
    let literal = unquote_literal(node_text(node, state.source));
    if !is_reference_literal(literal) {
        return;
    }
    if !state.string_refs_seen.insert(literal.to_lowercase()) {
        return;
    }
    let line = node.start_position().row as u32;
    let ref_id = make_node_id(&["str", literal.to_lowercase().as_str()]);
    state.nodes.push(ExtractedNode {
        id: ref_id.clone(),
        label: literal.to_string(),
        source_file: state.file_path.clone(),
        source_line: Some(line),
        docstring: None,
        signature: None,
        node_type: "reference".to_string(),
    });
    state.edges.push(ExtractedEdge {
        source: state.file_id.clone(),
        target: ref_id,
        relation: "references".to_string(),
        confidence: "EXTRACTED".to_string(),
        confidence_score: Some(1.0),
        source_file: state.file_path.clone(),
        source_line: Some(line),
    });
}

pub(crate) fn walk_structural<'a>(state: &mut ExtractionState<'a>, node: &Node<'a>) {
    let kind = node.kind();

    // --- Identifier-shaped string literals ---
    // Kinds whose name ends in "string" are literals in every grammar we
    // support (string, string_literal, interpreted_string_literal,
    // template_string, ...) while type annotations ("string_type") are
    // excluded by the suffix rule. Literals that look like identifiers
    // (env vars, snake/dotted/kebab/slash keys) become global reference
    // nodes so agents can trace where a config key or status value is used.
    if kind.ends_with("string") {
        collect_string_refs(state, node);
        return;
    }

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
                    signature: node_signature(node, state.source, state.cfg),
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
                    signature: node_signature(node, state.source, state.cfg),
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
            // Skip language builtins: they resolve to bare-name stubs that
            // merge corpus-wide and pollute god-node rankings.
            if !is_language_builtin(&name, state.cfg.name) {
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
            signature: None,
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

pub(crate) fn extract_single(
    path: &Path,
    cfg: &LanguageConfig,
) -> Result<Extraction, GraphifyError> {
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
        string_refs_seen: HashSet::new(),
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
        signature: None,
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
