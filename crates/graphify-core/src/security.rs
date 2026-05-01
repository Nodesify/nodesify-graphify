use std::path::Path;

const MAX_TEXT_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
const MAX_BINARY_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
const MAX_LABEL_LEN: usize = 256;
const MAX_DOCSTRING_LEN: usize = 4096;

/// Validate that a file path stays within the project root.
pub fn validate_path(root: &Path, file_path: &Path) -> Result<(), String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("invalid root: {}", e))?;
    let parent = file_path.parent().unwrap_or(file_path);
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("invalid path: {}", e))?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(format!(
            "path escapes project root: {}",
            file_path.display()
        ));
    }
    Ok(())
}

/// Check if a file is within acceptable size limits for processing.
pub fn check_file_size(path: &Path, size_bytes: u64) -> bool {
    if size_bytes > MAX_BINARY_FILE_SIZE {
        return false;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let text_extensions = [
        "py", "js", "jsx", "mjs", "ts", "tsx", "rs", "go", "java", "c", "h", "cpp", "cc", "cxx",
        "hpp", "md", "mdx", "txt", "rst", "toml", "yaml", "yml", "json", "xml", "rb", "swift",
        "kt", "scala", "php", "cs", "lua", "hs", "ex", "sh", "bash", "dart", "zig", "css", "html",
        "sql",
    ];
    let ext_lower = ext.to_lowercase();
    if text_extensions.contains(&ext_lower.as_str()) && size_bytes > MAX_TEXT_FILE_SIZE {
        return false;
    }
    true
}

/// Sanitize a node label: strip control chars and cap length.
pub fn sanitize_label(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(MAX_LABEL_LEN)
        .collect();
    cleaned
}

/// Sanitize a docstring: strip control chars and cap length.
pub fn sanitize_docstring(doc: &str) -> String {
    let cleaned: String = doc
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(MAX_DOCSTRING_LEN)
        .collect();
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_label_strips_control_chars() {
        let input = "hello\x00world\x01!";
        assert_eq!(sanitize_label(input), "helloworld!");
    }

    #[test]
    fn sanitize_label_caps_length() {
        let input: String = "x".repeat(500);
        assert_eq!(sanitize_label(&input).len(), MAX_LABEL_LEN);
    }

    #[test]
    fn check_file_size_allows_normal_files() {
        use std::path::PathBuf;
        assert!(check_file_size(&PathBuf::from("main.py"), 1024));
        assert!(check_file_size(
            &PathBuf::from("main.py"),
            MAX_TEXT_FILE_SIZE
        ));
        assert!(!check_file_size(
            &PathBuf::from("main.py"),
            MAX_TEXT_FILE_SIZE + 1
        ));
    }

    #[test]
    fn check_file_size_allows_larger_binary() {
        use std::path::PathBuf;
        assert!(check_file_size(
            &PathBuf::from("image.png"),
            MAX_TEXT_FILE_SIZE + 1
        ));
        assert!(!check_file_size(
            &PathBuf::from("image.png"),
            MAX_BINARY_FILE_SIZE + 1
        ));
    }
}
