use std::path::{Path, PathBuf};

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser};

use crate::langs::{self, LanguageConfig};
use crate::schema::{Extraction, ExtractedEdge, ExtractedNode};
use graphify_core::GraphifyError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Join parts with `_`, keep only lowercase alphanumeric and underscores.
fn make_id(parts: &[&str]) -> String {
    parts
        .join("_")
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
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

/// SHA-256 hash of the file contents.
fn file_hash(path: &Path) -> Result<String, GraphifyError> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Check the extraction_cache table. Returns cached Extraction if hit.
fn check_cache(db: &Connection, path: &Path, hash: &str) -> Option<Extraction> {
    let path_str = path.to_string_lossy();
    let mut stmt = db
        .prepare(
            "SELECT language, nodes, edges FROM extraction_cache WHERE file_path = ?1 AND content_hash = ?2",
        )
        .ok()?;
    stmt.query_row(rusqlite::params![path_str.as_ref(), hash], |row| {
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
    let path_str = path.to_string_lossy();
    let nodes_json = serde_json::to_string(&extraction.nodes).unwrap_or_default();
    let edges_json = serde_json::to_string(&extraction.edges).unwrap_or_default();
    let now = chrono_free_timestamp();

    let _ = db.execute(
        "INSERT OR REPLACE INTO extraction_cache (file_path, content_hash, language, nodes, edges, extracted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            path_str.as_ref(),
            hash,
            extraction.language,
            nodes_json,
            edges_json,
            now,
        ],
    );
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
    for child in body.children(&mut cursor) {
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
        // Stop after the first statement in the body
        break;
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
        let import_text = node_text(node, state.source);
        let module_name = extract_import_module(import_text, kind);
        if let Some(mod_name) = module_name {
            let target_id = make_id(&[&mod_name]);
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
        // Still walk children for nested structures
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_structural(state, &child);
        }
        return;
    }

    // --- Classes / structs / enums ---
    if state.cfg.class_types.contains(&kind) {
        let name_node = node.child_by_field_name(state.cfg.name_field);
        if let Some(name_node) = name_node {
            let name = node_text(&name_node, state.source).to_string();
            let class_id = make_id(&[&state.file_id, &name]);
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

    // --- Functions / methods ---
    if state.cfg.function_types.contains(&kind) {
        let name_node = node.child_by_field_name(state.cfg.name_field);
        if let Some(name_node) = name_node {
            let name = node_text(&name_node, state.source).to_string();
            let func_label = format!("{}()", name);
            let parent_id = state.current_class_id.as_deref().unwrap_or(&state.file_id);
            let func_id = make_id(&[parent_id, &name]);

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
fn extract_import_module(text: &str, kind: &str) -> Option<String> {
    // Python: "import foo" or "from foo import bar"
    if kind == "import_statement" || kind == "import_from_statement" {
        let cleaned = text
            .trim()
            .trim_start_matches("import ")
            .trim_start_matches("from ");
        let first = cleaned.split_whitespace().next()?;
        let module = first.split('.').next()?;
        return Some(module.to_string());
    }
    // JS/TS: import ... from 'module'
    if kind == "import_statement" || kind == "import_declaration" {
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
        return None;
    }
    // Rust: use foo::bar;
    if kind == "use_declaration" {
        let cleaned = text.trim_start_matches("use").trim().trim_end_matches(';');
        let first = cleaned.split("::").next()?.trim();
        if !first.is_empty() {
            return Some(first.to_string());
        }
        return None;
    }
    // Go: import "module"
    if kind == "import_declaration" {
        let cleaned = text.trim_start_matches("import").trim();
        let module = cleaned
            .trim_start_matches('"')
            .split(&['"', '\n'][..])
            .next()
            .unwrap_or("");
        if !module.is_empty() {
            return Some(module.to_string());
        }
        return None;
    }
    // Java: import foo.bar;
    if kind == "import_declaration" {
        let cleaned = text
            .trim_start_matches("import")
            .trim_start_matches("static")
            .trim()
            .trim_end_matches(';');
        let parts: Vec<&str> = cleaned.split('.').collect();
        if !parts.is_empty() {
            return Some(parts.join("."));
        }
        return None;
    }
    // C/C++: #include <foo> or #include "foo"
    if kind == "preproc_include" {
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
        return None;
    }
    None
}

// ---------------------------------------------------------------------------
// Pass 2: Call-graph extraction (walked inline during pass 1)
// ---------------------------------------------------------------------------

fn walk_calls<'a>(state: &mut ExtractionState<'a>, caller_id: &str, body: &Node<'a>) {
    let kind = body.kind();

    if kind == state.cfg.call_type {
        let callee_name = extract_callee_name(body, state.source);
        if let Some(name) = callee_name {
            let callee_id = make_id(&[&name]);
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
fn extract_callee_name<'a>(call_node: &Node, source: &'a [u8]) -> Option<String> {
    // The first child (field "function") is the callee
    let mut cursor = call_node.walk();
    let func_child = call_node.children(&mut cursor).next()?;

    let text = node_text(&func_child, source);
    // For method calls like obj.method(), take the last part
    let name = if text.contains('.') {
        text.split('.').last().unwrap_or(text)
    } else {
        text
    };
    Some(name.to_string())
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
    let file_id = make_id(&[&fid]);

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

    Ok(Extraction {
        file_path: path.to_path_buf(),
        language: cfg.name.to_string(),
        nodes: state.nodes,
        edges: state.edges,
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn extract(files: &[PathBuf], db: &Connection) -> Result<Vec<Extraction>, GraphifyError> {
    let mut results = Vec::new();

    for file_path in files {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let cfg = match langs::get_language_for_extension(ext) {
            Some(c) => c,
            None => continue,
        };

        let hash = file_hash(file_path)?;

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

    Ok(results)
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
}
