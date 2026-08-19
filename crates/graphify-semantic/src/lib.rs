// graphify-semantic: LLM-based semantic extraction for knowledge graph
// enrichment. Multi-backend (parity with upstream graphify v8 llm.py):
// Anthropic Claude, any OpenAI-compatible endpoint (OpenAI, DeepSeek,
// Ollama, custom providers), and Google Gemini — plus vision/image
// extraction through each backend's multimodal API.

use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use graphify_core::GraphifyError;
use graphify_core::Result;

/// Maximum image size sent to vision endpoints (5 MB, matching upstream).
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

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
    fn extract_semantic(&self, content: &str, file_type: &str) -> Result<SemanticExtraction>;

    /// Vision extraction: describe concepts from image bytes (PNG/JPEG/
    /// WEBP/GIF). Backends without multimodal support return an error.
    fn extract_semantic_from_image(
        &self,
        _image_bytes: &[u8],
        _media_type: &str,
    ) -> Result<SemanticExtraction> {
        Err(GraphifyError::Graph(
            "this backend does not support image extraction".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Shared prompt + parsing
// ---------------------------------------------------------------------------

fn system_prompt(file_type: &str) -> String {
    format!(
        "You are a knowledge graph extraction engine. Given the following {file_type} content, \
         extract semantic topics, concepts, and entities as nodes, and the relationships \
         between them as edges. Respond ONLY with valid JSON in this exact format:\n\
         {{\"nodes\": [{{\"id\": \"...\", \"label\": \"...\", \"summary\": \"...\", \"node_type\": \"...\"}}], \
         \"edges\": [{{\"source\": \"node_id\", \"target\": \"node_id\", \"relation\": \"...\"}}]}}\n\
         Use concise lowercase IDs (e.g. \"error_handling\"). \
         node_type should be one of: concept, entity, pattern, module, function.\n\
         relation should be one of: depends_on, implements, relates_to, contains, uses.\n\
         Return an empty JSON object if the content is too short or uninformative."
    )
}

fn vision_prompt() -> String {
    "You are a knowledge graph extraction engine. The user message contains an image \
     (a screenshot, diagram, whiteboard photo, chart, or slide). Extract the visible \
     concepts, entities, and their relationships as a knowledge graph. Respond ONLY \
     with valid JSON in the same format used for text extraction: \
     {\"nodes\": [...], \"edges\": [...]}. If the image has no meaningful content, \
     return an empty JSON object."
        .to_string()
}

/// Parse the model's text reply into an extraction, tolerating surrounding
/// prose by falling back to the outermost {...} span.
fn parse_extraction_text(text: &str) -> SemanticExtraction {
    if text.trim().is_empty() {
        return SemanticExtraction::empty();
    }
    if let Ok(parsed) = serde_json::from_str::<SemanticExtraction>(text.trim()) {
        return parsed;
    }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if let Ok(parsed) = serde_json::from_str::<SemanticExtraction>(&text[start..=end]) {
            return parsed;
        }
    }
    SemanticExtraction::empty()
}

/// POST with exponential backoff on 429/5xx, shared by all backends.
fn post_json(
    agent: &ureq::Agent,
    url: &str,
    headers: &[(&str, &str)],
    body: &str,
    backend_name: &str,
) -> Result<String> {
    let max_retries = 3;
    let mut last_err = None;
    for attempt in 0..=max_retries {
        let mut request = agent.post(url);
        for &(k, v) in headers {
            request = request.header(k, v);
        }
        match request.send(body) {
            Ok(resp) => {
                let status = resp.status();
                let response_body = resp.into_body().read_to_string().unwrap_or_default();
                if status.is_client_error() && status.as_u16() != 429 {
                    return Err(GraphifyError::Graph(format!(
                        "{backend_name} API returned {status}: {response_body}"
                    )));
                }
                if status.is_server_error() || status.as_u16() == 429 {
                    last_err = Some(format!("{backend_name} API returned {status}"));
                    if attempt < max_retries {
                        std::thread::sleep(Duration::from_millis(500 * 2u64.pow(attempt as u32)));
                        continue;
                    }
                    return Err(GraphifyError::Graph(last_err.unwrap()));
                }
                return Ok(response_body);
            }
            Err(e) => {
                last_err = Some(format!("{backend_name} API request failed: {e}"));
                if attempt < max_retries {
                    std::thread::sleep(Duration::from_millis(500 * 2u64.pow(attempt as u32)));
                    continue;
                }
            }
        }
    }
    Err(GraphifyError::Graph(last_err.unwrap()))
}

fn build_agent() -> ureq::Agent {
    ureq::config::Config::builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .new_agent()
}

// ---------------------------------------------------------------------------
// NoopBackend
// ---------------------------------------------------------------------------

/// A no-op backend that always returns empty extractions.
pub struct NoopBackend;

impl SemanticBackend for NoopBackend {
    fn extract_semantic(&self, _content: &str, _file_type: &str) -> Result<SemanticExtraction> {
        Ok(SemanticExtraction::empty())
    }
}

// ---------------------------------------------------------------------------
// ClaudeBackend (Anthropic Messages API)
// ---------------------------------------------------------------------------

pub struct ClaudeBackend {
    agent: ureq::Agent,
    api_key: String,
    model: String,
}

impl ClaudeBackend {
    /// - `GRAPHIFY_LLM_API_KEY` — required, the Anthropic API key.
    /// - `GRAPHIFY_LLM_MODEL` — optional, defaults to `claude-sonnet-4-20250514`.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GRAPHIFY_LLM_API_KEY").map_err(|_| {
            GraphifyError::Graph("GRAPHIFY_LLM_API_KEY environment variable is not set".into())
        })?;
        let model = std::env::var("GRAPHIFY_LLM_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
        Ok(Self::new(api_key, model))
    }

    pub fn new(api_key: String, model: String) -> Self {
        Self {
            agent: build_agent(),
            api_key,
            model,
        }
    }

    pub fn build_request_body(&self, content: &str, file_type: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system_prompt(file_type),
            "messages": [
                {"role": "user", "content": content}
            ]
        })
    }

    pub fn build_image_request_body(&self, image_b64: &str, media_type: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": vision_prompt(),
            "messages": [
                {"role": "user", "content": [
                    {"type": "image", "source": {"type": "base64", "media_type": media_type, "data": image_b64}},
                    {"type": "text", "text": "Extract the knowledge graph from this image."}
                ]}
            ]
        })
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("Content-Type", "application/json".into()),
            ("x-api-key", self.api_key.clone()),
            ("anthropic-version", "2023-06-01".into()),
        ]
    }
}

impl SemanticBackend for ClaudeBackend {
    fn extract_semantic(&self, content: &str, file_type: &str) -> Result<SemanticExtraction> {
        let body = serde_json::to_string(&self.build_request_body(content, file_type))?;
        let headers = self.headers();
        let hdr: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let response = post_json(
            &self.agent,
            "https://api.anthropic.com/v1/messages",
            &hdr,
            &body,
            "Claude",
        )?;
        let json: serde_json::Value = serde_json::from_str(&response).map_err(|e| {
            GraphifyError::Graph(format!("Failed to parse Claude API response: {e}"))
        })?;
        let text = json
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        Ok(parse_extraction_text(text))
    }

    fn extract_semantic_from_image(
        &self,
        image_bytes: &[u8],
        media_type: &str,
    ) -> Result<SemanticExtraction> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let body = serde_json::to_string(&self.build_image_request_body(&encoded, media_type))?;
        let headers = self.headers();
        let hdr: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let response = post_json(
            &self.agent,
            "https://api.anthropic.com/v1/messages",
            &hdr,
            &body,
            "Claude",
        )?;
        let json: serde_json::Value = serde_json::from_str(&response).map_err(|e| {
            GraphifyError::Graph(format!("Failed to parse Claude API response: {e}"))
        })?;
        let text = json
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        Ok(parse_extraction_text(text))
    }
}

// ---------------------------------------------------------------------------
// OpenAiBackend (any OpenAI-compatible endpoint)
// ---------------------------------------------------------------------------

/// Works with OpenAI, DeepSeek, Ollama (/v1), LM Studio, and custom
/// OpenAI-compatible providers via a configurable base URL.
pub struct OpenAiBackend {
    agent: ureq::Agent,
    api_key: Option<String>,
    base_url: String,
    model: String,
}

impl OpenAiBackend {
    /// - `GRAPHIFY_LLM_BASE_URL` (or `OPENAI_BASE_URL`) — defaults to OpenAI.
    /// - `GRAPHIFY_LLM_API_KEY` (or `OPENAI_API_KEY`) — optional for local
    ///   servers like Ollama.
    /// - `GRAPHIFY_LLM_MODEL` — defaults to `gpt-4o-mini`.
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("GRAPHIFY_LLM_BASE_URL")
            .or_else(|_| std::env::var("OPENAI_BASE_URL"))
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        let api_key = std::env::var("GRAPHIFY_LLM_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .ok();
        let model = std::env::var("GRAPHIFY_LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
        if api_key.is_none() && base_url.contains("api.openai.com") {
            return Err(GraphifyError::Graph(
                "no API key: set GRAPHIFY_LLM_API_KEY or OPENAI_API_KEY".into(),
            ));
        }
        Ok(Self {
            agent: build_agent(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        })
    }

    pub fn new(api_key: Option<String>, base_url: String, model: String) -> Self {
        Self {
            agent: build_agent(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        }
    }

    pub fn build_request_body(&self, content: &str, file_type: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                {"role": "system", "content": system_prompt(file_type)},
                {"role": "user", "content": content}
            ]
        })
    }

    pub fn build_image_request_body(&self, image_b64: &str, media_type: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                {"role": "system", "content": vision_prompt()},
                {"role": "user", "content": [
                    {"type": "text", "text": "Extract the knowledge graph from this image."},
                    {"type": "image_url", "image_url": {"url": format!("data:{media_type};base64,{image_b64}")}}
                ]}
            ]
        })
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = vec![("Content-Type", "application/json".to_string())];
        if let Some(key) = &self.api_key {
            headers.push(("Authorization", format!("Bearer {key}")));
        }
        headers
    }

    fn extract(&self, body: serde_json::Value) -> Result<SemanticExtraction> {
        let body_str = serde_json::to_string(&body)?;
        let url = format!("{}/chat/completions", self.base_url);
        let headers = self.headers();
        let hdr: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let response = post_json(&self.agent, &url, &hdr, &body_str, "OpenAI-compatible")?;
        let json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| GraphifyError::Graph(format!("Failed to parse OpenAI response: {e}")))?;
        let text = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        Ok(parse_extraction_text(text))
    }
}

impl SemanticBackend for OpenAiBackend {
    fn extract_semantic(&self, content: &str, file_type: &str) -> Result<SemanticExtraction> {
        self.extract(self.build_request_body(content, file_type))
    }

    fn extract_semantic_from_image(
        &self,
        image_bytes: &[u8],
        media_type: &str,
    ) -> Result<SemanticExtraction> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        self.extract(self.build_image_request_body(&encoded, media_type))
    }
}

// ---------------------------------------------------------------------------
// GeminiBackend (Google Generative Language API)
// ---------------------------------------------------------------------------

pub struct GeminiBackend {
    agent: ureq::Agent,
    api_key: String,
    model: String,
}

impl GeminiBackend {
    /// - `GRAPHIFY_LLM_API_KEY` (or `GEMINI_API_KEY`/`GOOGLE_API_KEY`).
    /// - `GRAPHIFY_LLM_MODEL` — defaults to `gemini-2.0-flash`.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GRAPHIFY_LLM_API_KEY")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
            .or_else(|_| std::env::var("GOOGLE_API_KEY"))
            .map_err(|_| {
                GraphifyError::Graph(
                    "no Gemini API key: set GRAPHIFY_LLM_API_KEY or GEMINI_API_KEY".into(),
                )
            })?;
        let model =
            std::env::var("GRAPHIFY_LLM_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".into());
        Ok(Self::new(api_key, model))
    }

    pub fn new(api_key: String, model: String) -> Self {
        Self {
            agent: build_agent(),
            api_key,
            model,
        }
    }

    pub fn url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        )
    }

    pub fn build_request_body(&self, content: &str, file_type: &str) -> serde_json::Value {
        serde_json::json!({
            "system_instruction": {"parts": [{"text": system_prompt(file_type)}]},
            "contents": [{"role": "user", "parts": [{"text": content}]}],
            "generationConfig": {"maxOutputTokens": 4096}
        })
    }

    pub fn build_image_request_body(&self, image_b64: &str, media_type: &str) -> serde_json::Value {
        serde_json::json!({
            "system_instruction": {"parts": [{"text": vision_prompt()}]},
            "contents": [{"role": "user", "parts": [
                {"inline_data": {"mime_type": media_type, "data": image_b64}},
                {"text": "Extract the knowledge graph from this image."}
            ]}],
            "generationConfig": {"maxOutputTokens": 4096}
        })
    }

    fn extract(&self, body: serde_json::Value) -> Result<SemanticExtraction> {
        let body_str = serde_json::to_string(&body)?;
        let response = post_json(
            &self.agent,
            &self.url(),
            &[("Content-Type", "application/json")],
            &body_str,
            "Gemini",
        )?;
        let json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| GraphifyError::Graph(format!("Failed to parse Gemini response: {e}")))?;
        let text = json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        Ok(parse_extraction_text(text))
    }
}

impl SemanticBackend for GeminiBackend {
    fn extract_semantic(&self, content: &str, file_type: &str) -> Result<SemanticExtraction> {
        self.extract(self.build_request_body(content, file_type))
    }

    fn extract_semantic_from_image(
        &self,
        image_bytes: &[u8],
        media_type: &str,
    ) -> Result<SemanticExtraction> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        self.extract(self.build_image_request_body(&encoded, media_type))
    }
}

// ---------------------------------------------------------------------------
// Backend resolution
// ---------------------------------------------------------------------------

/// Resolve the semantic backend from the environment.
///
/// `GRAPHIFY_LLM_BACKEND` selects explicitly (`claude`/`anthropic`,
/// `openai`/`openai-compatible`, `gemini`); otherwise backends are
/// auto-detected from which variables are set, in the order
/// Claude → OpenAI-compatible → Gemini.
pub fn backend_from_env() -> Result<Box<dyn SemanticBackend>> {
    let explicit = std::env::var("GRAPHIFY_LLM_BACKEND").unwrap_or_default();
    let explicit = explicit.trim().to_lowercase();
    match explicit.as_str() {
        "claude" | "anthropic" => return Ok(Box::new(ClaudeBackend::from_env()?)),
        "openai" | "openai-compatible" | "openai_compatible" => {
            return Ok(Box::new(OpenAiBackend::from_env()?))
        }
        "gemini" | "google" => return Ok(Box::new(GeminiBackend::from_env()?)),
        "" => {}
        other => {
            return Err(GraphifyError::Graph(format!(
                "unknown GRAPHIFY_LLM_BACKEND '{other}' (expected claude, openai, or gemini)"
            )))
        }
    }

    if std::env::var("GRAPHIFY_LLM_PROVIDER").is_ok()
        && std::env::var("GRAPHIFY_LLM_BASE_URL").is_ok()
    {
        return Ok(Box::new(OpenAiBackend::from_env()?));
    }
    // GRAPHIFY_LLM_API_KEY alone historically meant Claude; a base URL or
    // Gemini/Google key steers elsewhere.
    if std::env::var("GRAPHIFY_LLM_BASE_URL").is_ok() {
        return Ok(Box::new(OpenAiBackend::from_env()?));
    }
    if (std::env::var("GEMINI_API_KEY").is_ok() || std::env::var("GOOGLE_API_KEY").is_ok())
        && std::env::var("GRAPHIFY_LLM_API_KEY").is_err()
    {
        return Ok(Box::new(GeminiBackend::from_env()?));
    }
    Ok(Box::new(ClaudeBackend::from_env()?))
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Image extensions routed to vision extraction.
pub fn is_image_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "gif")
    )
}

/// Media type for an image path (for multimodal payloads).
pub fn image_media_type(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

/// Read the given files and run semantic extraction on each using the
/// provided backend. Text files go through text extraction; image files go
/// through the backend's vision path. Files that cannot be read or exceed
/// limits are silently skipped. Returns `(file_path, extraction)` pairs to
/// preserve file provenance.
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
        if is_image_file(path) {
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            if bytes.len() > MAX_IMAGE_BYTES {
                continue;
            }
            match backend.extract_semantic_from_image(&bytes, image_media_type(path)) {
                Ok(extraction) => results.push((path.clone(), extraction)),
                Err(_) => continue,
            }
            continue;
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
        let result = backend.extract_semantic("content", "txt").unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn noop_backend_has_no_vision() {
        assert!(NoopBackend
            .extract_semantic_from_image(&[], "image/png")
            .is_err());
    }

    // -- Claude payloads --

    #[test]
    fn claude_request_body_structure() {
        let backend = ClaudeBackend::new("key".into(), "claude-sonnet-4-20250514".into());
        let body = backend.build_request_body("hello", "rust");
        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["messages"][0]["content"], "hello");
        assert!(body["system"].as_str().unwrap().contains("rust"));
    }

    #[test]
    fn claude_image_payload_structure() {
        let backend = ClaudeBackend::new("key".into(), "m".into());
        let body = backend.build_image_request_body("QUJD", "image/png");
        let block = &body["messages"][0]["content"][0];
        assert_eq!(block["type"], "image");
        assert_eq!(block["source"]["media_type"], "image/png");
        assert_eq!(block["source"]["data"], "QUJD");
        assert_eq!(body["messages"][0]["content"][1]["type"], "text");
    }

    // -- OpenAI-compatible payloads --

    #[test]
    fn openai_request_body_structure() {
        let backend = OpenAiBackend::new(
            Some("k".into()),
            "https://api.openai.com/v1".into(),
            "gpt-4o-mini".into(),
        );
        let body = backend.build_request_body("hello", "python");
        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "hello");
    }

    #[test]
    fn openai_image_payload_uses_data_url() {
        let backend = OpenAiBackend::new(Some("k".into()), "https://x/v1".into(), "m".into());
        let body = backend.build_image_request_body("QUJD", "image/jpeg");
        let part = &body["messages"][1]["content"][1];
        assert_eq!(part["type"], "image_url");
        assert_eq!(part["image_url"]["url"], "data:image/jpeg;base64,QUJD");
    }

    #[test]
    fn openai_local_endpoint_needs_no_key() {
        let backend = OpenAiBackend::new(None, "http://localhost:11434/v1".into(), "llama3".into());
        // local model, no Authorization header expected
        let body = backend.build_request_body("x", "txt");
        assert_eq!(body["model"], "llama3");
    }

    // -- Gemini payloads --

    #[test]
    fn gemini_request_body_structure() {
        let backend = GeminiBackend::new("k".into(), "gemini-2.0-flash".into());
        let body = backend.build_request_body("hello", "markdown");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "hello");
        assert!(body["system_instruction"]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .contains("markdown"));
        assert!(backend.url().contains("gemini-2.0-flash:generateContent"));
    }

    #[test]
    fn gemini_image_payload_uses_inline_data() {
        let backend = GeminiBackend::new("k".into(), "m".into());
        let body = backend.build_image_request_body("QUJD", "image/webp");
        assert_eq!(
            body["contents"][0]["parts"][0]["inline_data"]["mime_type"],
            "image/webp"
        );
        assert_eq!(
            body["contents"][0]["parts"][0]["inline_data"]["data"],
            "QUJD"
        );
    }

    // -- Resolution --

    /// Env-mutating tests must not run concurrently.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn backend_resolution_unknown_name_errors() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("GRAPHIFY_LLM_BACKEND", "gpt9");
        let err = match backend_from_env() {
            Err(e) => e,
            Ok(_) => panic!("expected error for unknown backend name"),
        };
        assert!(err.to_string().contains("unknown GRAPHIFY_LLM_BACKEND"));
        std::env::remove_var("GRAPHIFY_LLM_BACKEND");
    }

    #[test]
    fn backend_resolution_explicit_openai() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("GRAPHIFY_LLM_BACKEND", "openai");
        std::env::set_var("OPENAI_API_KEY", "test");
        // must not error (constructs the backend without network access)
        assert!(backend_from_env().is_ok());
        std::env::remove_var("GRAPHIFY_LLM_BACKEND");
        std::env::remove_var("OPENAI_API_KEY");
    }

    // -- Parsing --

    #[test]
    fn parse_extraction_tolerates_prose_around_json() {
        let text = "Here you go:\n{\"nodes\":[{\"id\":\"a\",\"label\":\"A\",\"summary\":\"s\",\"node_type\":\"concept\"}],\"edges\":[]}\nDone.";
        let parsed = parse_extraction_text(text);
        assert_eq!(parsed.nodes.len(), 1);
        assert_eq!(parsed.nodes[0].id, "a");
    }

    #[test]
    fn parse_extraction_empty_text() {
        assert!(parse_extraction_text("").nodes.is_empty());
        assert!(parse_extraction_text("no json here").nodes.is_empty());
    }

    // -- File routing --

    #[test]
    fn image_files_detected() {
        assert!(is_image_file(std::path::Path::new("photo.PNG")));
        assert!(is_image_file(std::path::Path::new("d.webp")));
        assert!(!is_image_file(std::path::Path::new("doc.md")));
        assert_eq!(image_media_type(std::path::Path::new("x.png")), "image/png");
        assert_eq!(
            image_media_type(std::path::Path::new("x.jpg")),
            "image/jpeg"
        );
    }

    #[test]
    fn extraction_roundtrip() {
        let ext = SemanticExtraction {
            nodes: vec![SemanticNode {
                id: "graph_algo".into(),
                label: "Graph Algorithm".into(),
                summary: "Traversal".into(),
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
        assert_eq!(parsed.nodes[0].id, "graph_algo");
        assert_eq!(parsed.edges[0].relation, "contains");
    }
}
