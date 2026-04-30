use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedNode {
    pub id: String,
    pub label: String,
    pub source_file: PathBuf,
    pub source_line: Option<u32>,
    pub docstring: Option<String>,
    pub node_type: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
    pub confidence_score: Option<f64>,
    pub source_file: PathBuf,
    pub source_line: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Extraction {
    pub file_path: PathBuf,
    pub language: String,
    pub nodes: Vec<ExtractedNode>,
    pub edges: Vec<ExtractedEdge>,
}
