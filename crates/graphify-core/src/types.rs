use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileType {
    Code,
    Document,
    Paper,
    Image,
    Video,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Code => "code",
            FileType::Document => "document",
            FileType::Paper => "paper",
            FileType::Image => "image",
            FileType::Video => "video",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "code" => Some(FileType::Code),
            "document" => Some(FileType::Document),
            "paper" => Some(FileType::Paper),
            "image" => Some(FileType::Image),
            "video" => Some(FileType::Video),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceLocation {
    pub line: u32,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Relation {
    Calls,
    Imports,
    Uses,
    Defines,
    Contains,
    Inherits,
    References,
    Rationale { tag: String },
}

impl Relation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Relation::Calls => "calls",
            Relation::Imports => "imports",
            Relation::Uses => "uses",
            Relation::Defines => "defines",
            Relation::Contains => "contains",
            Relation::Inherits => "inherits",
            Relation::References => "references",
            Relation::Rationale { .. } => "rationale",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "calls" => Some(Relation::Calls),
            "imports" => Some(Relation::Imports),
            "uses" => Some(Relation::Uses),
            "defines" => Some(Relation::Defines),
            "contains" => Some(Relation::Contains),
            "inherits" => Some(Relation::Inherits),
            "references" => Some(Relation::References),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Confidence {
    Extracted,
    Inferred,
    Ambiguous,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::Extracted => "EXTRACTED",
            Confidence::Inferred => "INFERRED",
            Confidence::Ambiguous => "AMBIGUOUS",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "EXTRACTED" => Some(Confidence::Extracted),
            "INFERRED" => Some(Confidence::Inferred),
            "AMBIGUOUS" => Some(Confidence::Ambiguous),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub file_type: FileType,
    pub source_file: PathBuf,
    pub source_location: Option<SourceLocation>,
    pub docstring: Option<String>,
    pub community: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub relation: Relation,
    pub confidence: Confidence,
    pub confidence_score: Option<f64>,
    pub source_file: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub community_count: usize,
    pub file_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_type_roundtrip() {
        for ft in [FileType::Code, FileType::Document, FileType::Paper, FileType::Image, FileType::Video] {
            assert_eq!(FileType::from_str(ft.as_str()), Some(ft));
        }
    }

    #[test]
    fn relation_roundtrip() {
        for rel in [
            Relation::Calls, Relation::Imports, Relation::Uses,
            Relation::Defines, Relation::Contains, Relation::Inherits,
            Relation::References,
        ] {
            assert_eq!(Relation::from_str(rel.as_str()), Some(rel));
        }
    }

    #[test]
    fn confidence_roundtrip() {
        for conf in [Confidence::Extracted, Confidence::Inferred, Confidence::Ambiguous] {
            assert_eq!(Confidence::from_str(conf.as_str()), Some(conf));
        }
    }

    #[test]
    fn node_serialization_roundtrip() {
        let node = Node {
            id: "main.py::MyClass::method".into(),
            label: "method()".into(),
            file_type: FileType::Code,
            source_file: PathBuf::from("src/main.py"),
            source_location: Some(SourceLocation { line: 42, column: Some(4) }),
            docstring: Some("Does a thing".into()),
            community: Some(1),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, node.id);
        assert_eq!(back.community, node.community);
    }
}
