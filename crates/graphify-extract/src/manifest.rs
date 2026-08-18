// Deterministic manifest ingestion: pyproject.toml, Cargo.toml, go.mod,
// package.json, pom.xml → one canonical `pkg_<name>` node per package plus
// `depends_on` edges. Ported from upstream graphify v8 manifest_ingest.py /
// cargo_introspect.py. Never uses an LLM — dependency structure is fact, not
// inference, and a shared dependency should emerge as a hub node.

use std::path::Path;

use graphify_core::ids::normalize_id;
use graphify_core::MANIFEST_FILENAMES;
use regex::Regex;

use crate::schema::{ExtractedEdge, ExtractedNode, Extraction};

pub fn is_manifest(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| MANIFEST_FILENAMES.contains(&n.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn pkg_id(name: &str) -> String {
    format!("pkg_{}", normalize_id(name))
}

struct ManifestBuilder {
    file_path: std::path::PathBuf,
    nodes: Vec<ExtractedNode>,
    edges: Vec<ExtractedEdge>,
}

impl ManifestBuilder {
    fn new(path: &Path) -> Self {
        Self {
            file_path: path.to_path_buf(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn add_package(&mut self, name: &str, docstring: Option<&str>) -> String {
        let id = pkg_id(name);
        if !self.nodes.iter().any(|n| n.id == id) {
            self.nodes.push(ExtractedNode {
                id: id.clone(),
                label: name.to_string(),
                source_file: self.file_path.clone(),
                source_line: Some(1),
                docstring: docstring.map(|s| s.to_string()),
                node_type: "package".to_string(),
            });
        }
        id
    }

    fn add_dependency(&mut self, from: &str, dep_name: &str) {
        let dep_id = self.add_package(dep_name, None);
        if from != dep_id {
            self.edges.push(ExtractedEdge {
                source: from.to_string(),
                target: dep_id,
                relation: "depends_on".to_string(),
                confidence: "EXTRACTED".to_string(),
                confidence_score: Some(1.0),
                source_file: self.file_path.clone(),
                source_line: Some(1),
            });
        }
    }
}

pub fn extract_manifest(path: &Path) -> Extraction {
    let mut builder = ManifestBuilder::new(path);
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let text = std::fs::read_to_string(path).unwrap_or_default();

    match filename.as_str() {
        "pyproject.toml" => ingest_pyproject(&text, &mut builder),
        "cargo.toml" => ingest_cargo(path, &text, &mut builder),
        "go.mod" => ingest_go_mod(&text, &mut builder),
        "package.json" => ingest_package_json(&text, &mut builder),
        "pom.xml" => ingest_pom(&text, &mut builder),
        _ => {}
    }

    Extraction {
        file_path: path.to_path_buf(),
        language: "Manifest".to_string(),
        nodes: builder.nodes,
        edges: builder.edges,
    }
}

fn ingest_pyproject(text: &str, b: &mut ManifestBuilder) {
    let Ok(doc) = text.parse::<toml::Table>() else {
        return;
    };
    let Some(project) = doc.get("project").and_then(|v| v.as_table()) else {
        return;
    };
    let name = project
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown-python-project");
    let self_id = b.add_package(name, Some("this project (pyproject.toml)"));
    if let Some(deps) = project.get("dependencies").and_then(|v| v.as_array()) {
        for dep in deps.iter().filter_map(|d| d.as_str()) {
            // PEP 508: take the name before the first version marker
            let dep_name = dep
                .split(|c: char| {
                    c == '='
                        || c == '<'
                        || c == '>'
                        || c == '~'
                        || c == '!'
                        || c == ';'
                        || c == '['
                        || c.is_whitespace()
                })
                .next()
                .unwrap_or("")
                .trim();
            if !dep_name.is_empty() {
                b.add_dependency(&self_id, dep_name);
            }
        }
    }
}

fn ingest_cargo(path: &Path, text: &str, b: &mut ManifestBuilder) {
    let Ok(doc) = text.parse::<toml::Table>() else {
        return;
    };

    if let Some(package) = doc.get("package").and_then(|v| v.as_table()) {
        let name = package
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown-crate");
        let self_id = b.add_package(name, Some("this crate (Cargo.toml)"));
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(deps) = package.get(section).and_then(|v| v.as_table()) {
                for dep_name in deps.keys() {
                    b.add_dependency(&self_id, dep_name);
                }
            }
        }
    }

    // Workspace: one node per member crate, linked with depends_on so the
    // internal crate graph emerges. Member names are read from each member's
    // own Cargo.toml when resolvable.
    if let Some(workspace) = doc.get("workspace").and_then(|v| v.as_table()) {
        let root_id = b.add_package(
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("workspace"),
            Some("cargo workspace"),
        );
        if let Some(members) = workspace.get("members").and_then(|v| v.as_array()) {
            for member in members.iter().filter_map(|m| m.as_str()) {
                let dir = member.trim_end_matches("/*").trim_end_matches('/');
                let member_toml = path.parent().unwrap_or(path).join(dir).join("Cargo.toml");
                let crate_name = std::fs::read_to_string(&member_toml)
                    .ok()
                    .and_then(|t| t.parse::<toml::Table>().ok())
                    .and_then(|d| {
                        d.get("package")
                            .and_then(|p| p.as_table())
                            .and_then(|p| p.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| dir.replace('/', "_"));
                let member_id = b.add_package(&crate_name, Some("workspace member"));
                if member_id != root_id {
                    b.edges.push(ExtractedEdge {
                        source: root_id.clone(),
                        target: member_id,
                        relation: "contains".to_string(),
                        confidence: "EXTRACTED".to_string(),
                        confidence_score: Some(1.0),
                        source_file: b.file_path.clone(),
                        source_line: Some(1),
                    });
                }
            }
        }
    }
}

fn ingest_go_mod(text: &str, b: &mut ManifestBuilder) {
    let mut module_name = None;
    let mut in_block = false;
    let mut requires: Vec<String> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("module ") {
            module_name = Some(rest.trim().to_string());
        }
        if line.starts_with("require (") {
            in_block = true;
            continue;
        }
        if in_block && line == ")" {
            in_block = false;
            continue;
        }
        let entry = if in_block {
            Some(line)
        } else {
            line.strip_prefix("require ").map(|r| r.trim())
        };
        if let Some(entry) = entry {
            let mut parts = entry.split_whitespace();
            if let Some(dep) = parts.next() {
                let indirect = parts
                    .last()
                    .map(|l| l.contains("indirect"))
                    .unwrap_or(false);
                if !indirect {
                    requires.push(dep.to_string());
                }
            }
        }
    }

    let self_id = b.add_package(
        module_name.as_deref().unwrap_or("unknown-go-module"),
        Some("this module (go.mod)"),
    );
    for dep in requires {
        b.add_dependency(&self_id, &dep);
    }
}

fn ingest_package_json(text: &str, b: &mut ManifestBuilder) {
    let Ok(doc) = text.parse::<serde_json::Value>() else {
        return;
    };
    let name = doc
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown-npm-package");
    let self_id = b.add_package(name, Some("this package (package.json)"));
    for section in ["dependencies", "devDependencies"] {
        if let Some(deps) = doc.get(section).and_then(|v| v.as_object()) {
            for dep_name in deps.keys() {
                b.add_dependency(&self_id, dep_name);
            }
        }
    }
}

fn ingest_pom(text: &str, b: &mut ManifestBuilder) {
    let artifact_re = Regex::new(r"<artifactId>\s*([^<]+?)\s*</artifactId>").unwrap();
    let deps_re = Regex::new(r"(?s)<dependencies>(.*?)</dependencies>").unwrap();

    let self_name = artifact_re
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unknown-maven-artifact".into());
    let self_id = b.add_package(&self_name, Some("this artifact (pom.xml)"));

    for dep_block in deps_re.captures_iter(text) {
        let block = dep_block.get(1).map(|m| m.as_str()).unwrap_or("");
        for cap in artifact_re.captures_iter(block) {
            if let Some(m) = cap.get(1) {
                b.add_dependency(&self_id, m.as_str());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyproject_produces_package_and_deps() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("pyproject.toml");
        std::fs::write(
            &p,
            "[project]\nname = \"my-app\"\ndependencies = [\"httpx>=0.27\", \"rich\"]\n",
        )
        .unwrap();
        let ext = extract_manifest(&p);
        assert_eq!(ext.language, "Manifest");
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_my_app"));
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_httpx"));
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_rich"));
        assert!(ext.edges.iter().any(|e| e.relation == "depends_on"
            && e.source == "pkg_my_app"
            && e.target == "pkg_httpx"));
    }

    #[test]
    fn package_json_deps_and_self() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("package.json");
        std::fs::write(
            &p,
            r#"{"name": "@scope/cli", "dependencies": {"commander": "^12.0.0"}}"#,
        )
        .unwrap();
        let ext = extract_manifest(&p);
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_scope_cli"));
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_commander"));
        assert_eq!(
            ext.edges
                .iter()
                .filter(|e| e.relation == "depends_on")
                .count(),
            1
        );
    }

    #[test]
    fn go_mod_skips_indirect() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("go.mod");
        std::fs::write(
            &p,
            "module example.com/app\n\ngo 1.22\n\nrequire (\n\tgithub.com/foo/bar v1.0.0\n\tgithub.com/indirect/dep v0.1.0 // indirect\n)\n",
        )
        .unwrap();
        let ext = extract_manifest(&p);
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_example_com_app"));
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_github_com_foo_bar"));
        assert!(!ext.nodes.iter().any(|n| n.id.contains("indirect")));
    }

    #[test]
    fn cargo_workspace_members_read_names() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("Cargo.toml");
        std::fs::write(
            &ws,
            "[workspace]\nmembers = [\"crates/core\", \"crates/cli\"]\n",
        )
        .unwrap();
        let core = dir.path().join("crates/core/Cargo.toml");
        std::fs::create_dir_all(core.parent().unwrap()).unwrap();
        std::fs::write(
            &core,
            "[package]\nname = \"graphify-core\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        // cli member has no Cargo.toml — falls back to dir-derived name
        std::fs::create_dir_all(dir.path().join("crates/cli")).unwrap();

        let ext = extract_manifest(&ws);
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_graphify_core"));
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_crates_cli"));
        assert!(ext
            .edges
            .iter()
            .any(|e| e.relation == "contains" && e.target == "pkg_graphify_core"));
    }

    #[test]
    fn pom_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("pom.xml");
        std::fs::write(
            &p,
            "<project><artifactId>my-app</artifactId><dependencies><dependency><groupId>org.x</groupId><artifactId>junit</artifactId></dependency></dependencies></project>",
        )
        .unwrap();
        let ext = extract_manifest(&p);
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_my_app"));
        assert!(ext.nodes.iter().any(|n| n.id == "pkg_junit"));
    }

    #[test]
    fn manifest_detection_by_filename() {
        assert!(is_manifest(Path::new("any/dir/pyproject.toml")));
        assert!(is_manifest(Path::new("Cargo.TOML")));
        assert!(is_manifest(Path::new("go.mod")));
        assert!(!is_manifest(Path::new("main.rs")));
        assert!(!is_manifest(Path::new("README.md")));
    }
}
