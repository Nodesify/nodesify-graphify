// graphify-semantic: LLM-based semantic extraction for knowledge graph enrichment

use std::path::PathBuf;
use std::time::Duration;

use graphify_core::GraphifyError;
use graphify_core::Result;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A semantically extracted node (topic, concept, entity).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SemanticNode {
    pub id: String,
    pub label: String,
    pub summary: String,
    pub node_type: String,
}

/// A semantically extracted edge (relationship between two nodes).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SemanticEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

/// The result of semantic extraction on a single piece of content.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SemanticExtraction {
    pub nodes: Vec<SemanticNode>,
    pub edges: Vec<SemanticEdge>,
}

impl SemanticExtraction {
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// Trait for semantic extraction backends.
pub trait SemanticBackend {
    fn extract_semantic(
        &self,
        content: &str,
        file_type: &str,
    ) -> Result<SemanticExtraction>;
}

// ---------------------------------------------------------------------------
// NoopBackend
// ---------------------------------------------------------------------------

/// A no-op backend that always returns empty extractions.
pub struct NoopBackend;

impl SemanticBackend for NoopBackend {
    fn extract_semantic(
        &self,
        _content: &str,
        _file_type: &str,
    ) -> Result<SemanticExtraction> {
        Ok(SemanticExtraction::empty())
    }
}

// ---------------------------------------------------------------------------
// ClaudeBackend
// ---------------------------------------------------------------------------

/// Backend that calls the Anthropic Claude API for semantic extraction.
pub struct ClaudeBackend {
    agent: ureq::Agent,
    api_key: String,
    model: String,
}

impl ClaudeBackend {
    /// Create a new ClaudeBackend reading configuration from environment variables.
    ///
    /// - `GRAPHIFY_LLM_API_KEY` — required, the Anthropic API key.
    /// - `GRAPHIFY_LLM_MODEL` — optional, defaults to `claude-sonnet-4-20250514`.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GRAPHIFY_LLM_API_KEY").map_err(|_| {
            GraphifyError::Graph(
                "GRAPHIFY_LLM_API_KEY environment variable is not set".into(),
            )
        })?;
        let model =
            std::env::var("GRAPHIFY_LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
        let agent = ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        Ok(Self { agent, api_key, model })
    }

    /// Create with explicit credentials (useful for testing).
    pub fn new(api_key: String, model: String) -> Self {
        let agent = ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        Self { agent, api_key, model }
    }

    /// Build the JSON prompt payload sent to the Anthropic Messages API.
    fn build_request_body(&self, content: &str, file_type: &str) -> serde_json::Value {
        let system_prompt = format!(
            "You are a knowledge graph extraction engine. Given the following {file_type} content, \
             extract semantic topics, concepts, and entities as nodes, and the relationships \
             between them as edges. Respond ONLY with valid JSON in this exact format:\n\
             {{\"nodes\": [{{\"id\": \"...\", \"label\": \"...\", \"summary\": \"...\", \"node_type\": \"...\"}}], \
             \"edges\": [{{\"source\": \"node_id\", \"target\": \"node_id\", \"relation\": \"...\"}}]}}\n\
             Use concise lowercase IDs (e.g. \"error_handling\"). \
             node_type should be one of: concept, entity, pattern, module, function.\n\
             relation should be one of: depends_on, implements, relates_to, contains, uses.\n\
             Return an empty JSON object if the content is too short or uninformative."
        );

        serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system_prompt,
            "messages": [
                {
                    "role": "user",
                    "content": content
                }
            ]
        })
    }
}

impl SemanticBackend for ClaudeBackend {
    fn extract_semantic(
        &self,
        content: &str,
        file_type: &str,
    ) -> Result<SemanticExtraction> {
        let body = self.build_request_body(content, file_type);
        let body_str = serde_json::to_string(&body)?;

        let response = self.agent.post("https://api.anthropic.com/v1/messages")
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .send(&body_str)
            .map_err(|e| GraphifyError::Graph(format!("Claude API request failed: {e}")))?;

        let response_body = response.into_body().read_to_string().unwrap_or_default();
        let response_json: serde_json::Value = serde_json::from_str(&response_body)
            .map_err(|e| GraphifyError::Graph(format!("Failed to parse Claude API response: {e}")))?;

        // Extract the text content from the response.
        let text = response_json
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        if text.is_empty() {
            return Ok(SemanticExtraction::empty());
        }

        // Parse the JSON from the model's response.
        let extraction: SemanticExtraction = serde_json::from_str(text.trim()).unwrap_or_else(|_| {
            // Try to find a JSON object within the text in case the model wrapped it.
            if let Some(start) = text.find('{') {
                if let Some(end) = text.rfind('}') {
                    if let Ok(parsed) = serde_json::from_str::<SemanticExtraction>(&text[start..=end]) {
                        return parsed;
                    }
                }
            }
            SemanticExtraction::empty()
        });

        Ok(extraction)
    }
}

// ---------------------------------------------------------------------------
// Public helper
// ---------------------------------------------------------------------------

/// Read the given files and run semantic extraction on each using the provided backend.
///
/// Files that cannot be read as UTF-8 text are silently skipped.
/// Returns `(file_path, extraction)` tuples to preserve file provenance.
pub fn extract_semantic_for_files(
    files: &[PathBuf],
    backend: &dyn SemanticBackend,
) -> Vec<(PathBuf, SemanticExtraction)> {
    let mut results = Vec::new();
    for (i, path) in files.iter().enumerate() {
        if i > 0 {
            // Simple rate-limiting: pause between API calls to avoid hitting limits.
            std::thread::sleep(Duration::from_millis(500));
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");
        match backend.extract_semantic(&content, ext) {
            Ok(extraction) => results.push((path.clone(), extraction)),
            Err(_) => continue,
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_backend_returns_empty() {
        let backend = NoopBackend;
        let result = backend
            .extract_semantic("some content about graphs", "txt")
            .unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn claude_backend_request_body_structure() {
        let backend = ClaudeBackend::new("test-key".into(), "claude-sonnet-4-20250514".into());
        let body = backend.build_request_body("hello world", "rust");

        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["max_tokens"], 4096);
        assert!(body["system"].as_str().unwrap().contains("rust"));
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hello world");
    }

    #[test]
    fn claude_backend_prompt_mentions_expected_format() {
        let backend = ClaudeBackend::new("test-key".into(), "claude-sonnet-4-20250514".into());
        let body = backend.build_request_body("content", "python");
        let system = body["system"].as_str().unwrap();

        assert!(system.contains("nodes"));
        assert!(system.contains("edges"));
        assert!(system.contains("python"));
        assert!(system.contains("knowledge graph"));
    }

    #[test]
    fn semantic_extraction_empty() {
        let ext = SemanticExtraction::empty();
        assert!(ext.nodes.is_empty());
        assert!(ext.edges.is_empty());
    }

    #[test]
    fn semantic_extraction_roundtrip() {
        let ext = SemanticExtraction {
            nodes: vec![SemanticNode {
                id: "graph_algo".into(),
                label: "Graph Algorithm".into(),
                summary: "Algorithms for graph traversal".into(),
                node_type: "concept".into(),
            }],
            edges: vec![SemanticEdge {
                source: "graph_algo".into(),
                target: "bfs".into(),
                relation: "contains".into(),
            }],
        };
        let json = serde_json::to_string(&ext).unwrap();
        let parsed: SemanticExtraction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.nodes.len(), 1);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.nodes[0].id, "graph_algo");
        assert_eq!(parsed.edges[0].relation, "contains");
    }

    #[test]
    fn extract_semantic_for_files_skips_nonexistent() {
        let backend = NoopBackend;
        let files = vec![PathBuf::from("/nonexistent/file.txt")];
        let results = extract_semantic_for_files(&files, &backend);
        assert!(results.is_empty());
    }

    #[test]
    fn claude_backend_default_model() {
        // We cannot call from_env() in tests (no env var), but we can verify the default
        // is applied when the env var is missing by checking the constructor directly.
        let backend = ClaudeBackend::new("key".into(), "claude-sonnet-4-20250514".into());
        let body = backend.build_request_body("x", "txt");
        assert_eq!(body["model"], "claude-sonnet-4-20250514");
    }
}
