// graphify-ingest: URL ingestion for fetching and saving remote content

use std::io::Read;
use std::path::{Path, PathBuf};

use graphify_core::GraphifyError;
use graphify_core::Result;

/// Maximum download size: 50 MB.
const MAX_DOWNLOAD_BYTES: usize = 50 * 1024 * 1024;

/// Fetch a URL and save its content to `out_dir`.
///
/// Handles:
/// - Web pages (HTML stripped to text, saved as `.md`)
/// - arxiv PDFs (downloaded as `.pdf`)
/// - Plain text URLs (saved as-is)
///
/// Only `http://` and `https://` URLs are allowed.
/// Returns the path of the saved file.
pub fn fetch_url(url: &str, out_dir: &Path) -> Result<PathBuf> {
    // Validate scheme.
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(GraphifyError::Graph(format!(
            "Unsupported URL scheme (only http/https allowed): {url}"
        )));
    }

    // Block private/internal hosts to prevent SSRF.
    if let Some(host) = extract_host(url) {
        if is_private_host(&host) {
            return Err(GraphifyError::Graph(format!(
                "URL resolves to a private/internal address (blocked): {url}"
            )));
        }
    }

    std::fs::create_dir_all(out_dir)?;

    let filename = derive_filename(url);
    let out_path = out_dir.join(&filename);

    let response = ureq::get(url)
        .call()
        .map_err(|e| GraphifyError::Graph(format!("Failed to fetch {url}: {e}")))?;

    let mut reader = response.into_body().into_reader();
    let mut raw_bytes = Vec::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| GraphifyError::Graph(format!("Download read error: {e}")))?;
        if n == 0 {
            break;
        }
        raw_bytes.extend_from_slice(&buf[..n]);
        if raw_bytes.len() > MAX_DOWNLOAD_BYTES {
            return Err(GraphifyError::Graph(format!(
                "Download exceeded {} byte limit for {url}",
                MAX_DOWNLOAD_BYTES
            )));
        }
    }

    // Decide how to save based on content type / URL.
    if is_pdf_url(url) {
        std::fs::write(&out_path, &raw_bytes)?;
    } else if looks_like_html(&raw_bytes) {
        let text = html_to_text(&raw_bytes);
        let md_path = out_path.with_extension("md");
        std::fs::write(&md_path, text)?;
        return Ok(md_path);
    } else {
        // Treat as plain text.
        std::fs::write(&out_path, &raw_bytes)?;
    }

    Ok(out_path)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Derive a safe filename from the URL.
fn derive_filename(url: &str) -> String {
    // Try to use the last path segment.
    let path_part = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or("index");

    // If there's no path (just a domain), treat as root.
    if !path_part.contains('/') {
        return "index.html".into();
    }

    let last_segment = path_part
        .rsplit('/')
        .next()
        .unwrap_or("index")
        .to_string();

    if last_segment.is_empty() || last_segment == "/" {
        "index.html".into()
    } else {
        last_segment
    }
}

/// Check if the URL points to a PDF.
fn is_pdf_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.ends_with(".pdf")
        || lower.contains("arxiv.org/pdf/")
        || lower.contains("arxiv.org/abs/")
}

/// Heuristic check: does the content look like HTML?
fn looks_like_html(bytes: &[u8]) -> bool {
    let head = if bytes.len() > 512 {
        &bytes[..512]
    } else {
        bytes
    };
    let s = String::from_utf8_lossy(head).to_lowercase();
    s.contains("<html") || s.contains("<!doctype html") || s.contains("<head")
}

/// Very basic HTML to text conversion: strip tags, collapse whitespace.
fn html_to_text(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let mut result = String::with_capacity(raw.len());
    let mut in_tag = false;
    let mut last_was_space = false;

    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
            }
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !last_was_space {
                    result.push(c);
                    last_was_space = true;
                }
            }
            c => {
                result.push(c);
                last_was_space = false;
            }
        }
    }

    // Add a markdown-style title if we can extract one.
    let text = result.trim().to_string();
    if text.is_empty() {
        return "(empty page)".into();
    }

    text
}

/// Extract the host portion from a URL string.
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1)?;
    let authority = after_scheme.split('/').next()?;
    let host = authority.split(':').next()?;
    Some(host.to_lowercase())
}

/// Check whether a hostname points to a private or internal address.
fn is_private_host(host: &str) -> bool {
    // Localhost names.
    if host == "localhost" || host == "localhost.localdomain" || host.ends_with(".localhost") {
        return true;
    }

    // Try to parse as an IPv4 address.
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        let octets = ip.octets();
        // 0.0.0.0/8
        if octets[0] == 0 {
            return true;
        }
        // 127.0.0.0/8 (loopback)
        if octets[0] == 127 {
            return true;
        }
        // 10.0.0.0/8
        if octets[0] == 10 {
            return true;
        }
        // 172.16.0.0/12
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return true;
        }
        // 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return true;
        }
        // 169.254.0.0/16 (link-local)
        if octets[0] == 169 && octets[1] == 254 {
            return true;
        }
        // 100.64.0.0/10 (Carrier-grade NAT)
        if octets[0] == 100 && (64..=127).contains(&octets[1]) {
            return true;
        }
    }

    // IPv6 loopback and link-local.
    if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        if ip.is_loopback() {
            return true;
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_non_http_url() {
        let result = fetch_url("ftp://example.com/file", Path::new("/tmp"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Unsupported URL scheme"));
    }

    #[test]
    fn derive_filename_basic() {
        assert_eq!(derive_filename("https://example.com/page.html"), "page.html");
        assert_eq!(derive_filename("https://example.com/doc.pdf"), "doc.pdf");
    }

    #[test]
    fn derive_filename_root() {
        assert_eq!(derive_filename("https://example.com/"), "index.html");
        assert_eq!(derive_filename("https://example.com"), "index.html");
    }

    #[test]
    fn is_pdf_detection() {
        assert!(is_pdf_url("https://arxiv.org/pdf/2401.00001"));
        assert!(is_pdf_url("https://arxiv.org/abs/2401.00001"));
        assert!(is_pdf_url("https://example.com/paper.pdf"));
        assert!(!is_pdf_url("https://example.com/page.html"));
    }

    #[test]
    fn looks_like_html_detection() {
        assert!(looks_like_html(b"<html><body>Hello</body></html>"));
        assert!(looks_like_html(b"<!DOCTYPE html><head>"));
        assert!(!looks_like_html(b"Just some plain text"));
    }

    #[test]
    fn html_to_text_strips_tags() {
        let html = b"<html><head><title>Test</title></head><body><p>Hello world</p></body></html>";
        let text = html_to_text(html);
        assert!(!text.contains('<'));
        assert!(!text.contains('>'));
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
    }

    #[test]
    fn html_to_text_empty() {
        let html = b"<html><body></body></html>";
        let text = html_to_text(html);
        assert!(!text.is_empty()); // Should produce at least "(empty page)" or whitespace
    }

    #[test]
    fn max_download_size_constant() {
        assert_eq!(MAX_DOWNLOAD_BYTES, 50 * 1024 * 1024);
    }

    #[test]
    fn reject_localhost_url() {
        let result = fetch_url("http://localhost:8080/api", Path::new("/tmp"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("private"));
    }

    #[test]
    fn reject_private_ip_10() {
        let result = fetch_url("http://10.0.0.1/secret", Path::new("/tmp"));
        assert!(result.is_err());
    }

    #[test]
    fn reject_private_ip_172() {
        let result = fetch_url("http://172.16.5.1/internal", Path::new("/tmp"));
        assert!(result.is_err());
    }

    #[test]
    fn reject_private_ip_192() {
        let result = fetch_url("http://192.168.1.1/router", Path::new("/tmp"));
        assert!(result.is_err());
    }

    #[test]
    fn reject_loopback_127() {
        let result = fetch_url("http://127.0.0.1:3000/api", Path::new("/tmp"));
        assert!(result.is_err());
    }

    #[test]
    fn allow_public_url() {
        // Just test that the host check passes (will fail on network, but not SSRF block).
        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("github.com"));
        assert!(!is_private_host("93.184.216.34"));
    }

    #[test]
    fn extract_host_works() {
        assert_eq!(extract_host("https://example.com/path"), Some("example.com".into()));
        assert_eq!(extract_host("http://localhost:3000/api"), Some("localhost".into()));
        assert_eq!(extract_host("noscheme"), None);
    }
}
