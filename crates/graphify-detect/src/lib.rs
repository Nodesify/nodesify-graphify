// graphify-detect: file discovery, classification, and incremental detection

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use graphify_core::FileType;
use rusqlite::Connection;
use sha2::{Sha256, Digest};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub file_type: FileType,
    pub language: Option<String>,
    pub content_hash: String,
    pub size_bytes: u64,
}

#[derive(Debug)]
pub struct DetectResult {
    pub new: Vec<FileEntry>,
    pub changed: Vec<FileEntry>,
    pub unchanged: Vec<FileEntry>,
    pub removed: Vec<FileEntry>,
}

const CODE_EXTENSIONS: &[&str] = &[
    ".py", ".js", ".jsx", ".mjs", ".ts", ".tsx",
    ".rs", ".go", ".java", ".c", ".h", ".cpp", ".cc", ".cxx", ".hpp",
];
const DOC_EXTENSIONS: &[&str] = &[".md", ".mdx", ".txt", ".rst"];
const PAPER_EXTENSIONS: &[&str] = &[".pdf"];
const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"];
const VIDEO_EXTENSIONS: &[&str] = &[".mp4", ".mov", ".webm", ".mkv", ".avi"];

const EXTENSION_TO_LANGUAGE: &[(&str, &str)] = &[
    (".py", "Python"), (".js", "JavaScript"), (".jsx", "JavaScript"),
    (".mjs", "JavaScript"), (".ts", "TypeScript"), (".tsx", "TypeScript"),
    (".rs", "Rust"), (".go", "Go"), (".java", "Java"),
    (".c", "C"), (".h", "C"), (".cpp", "C++"), (".cc", "C++"),
    (".cxx", "C++"), (".hpp", "C++"),
];

pub fn classify_file(path: &Path) -> Option<FileType> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let ext_with_dot = format!(".{}", ext);
    if CODE_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Code);
    }
    if DOC_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Document);
    }
    if PAPER_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Paper);
    }
    if IMAGE_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Image);
    }
    if VIDEO_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Video);
    }
    None
}

pub fn language_for_extension(ext: &str) -> Option<&'static str> {
    let ext_with_dot = if ext.starts_with('.') { ext.to_lowercase() } else { format!(".{}", ext).to_lowercase() };
    EXTENSION_TO_LANGUAGE.iter()
        .find(|(e, _)| e.to_lowercase() == ext_with_dot)
        .map(|(_, lang)| *lang)
}

fn file_hash(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn detect(root: &Path, db: &Connection) -> graphify_core::Result<DetectResult> {
    let mut new_files = Vec::new();
    let mut changed_files = Vec::new();
    let mut unchanged_files = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();

    let mut ignore_builder = ignore::WalkBuilder::new(root);
    ignore_builder
        .hidden(false)
        .git_ignore(true)
        .add_custom_ignore_filename(".graphifyignore");

    let graphifyignore = root.join(".graphifyignore");
    if graphifyignore.exists() {
        let _ = ignore_builder.add_ignore(graphifyignore);
    }

    for entry in ignore_builder.build().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(path);
        let Some(file_type) = classify_file(path) else {
            continue;
        };

        let rel_str = relative.to_string_lossy().to_string().replace('\\', "/");
        seen_paths.insert(rel_str.clone());

        let metadata = std::fs::metadata(path)?;
        let size_bytes = metadata.len();
        let hash = file_hash(path)?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let language = language_for_extension(ext).map(|s| s.to_string());

        let stored_hash: Option<String> = db
            .query_row(
                "SELECT content_hash FROM file_manifest WHERE file_path = ?1",
                rusqlite::params![rel_str],
                |row| row.get(0),
            )
            .ok();

        let entry = FileEntry {
            path: relative.to_path_buf(),
            file_type,
            language,
            content_hash: hash,
            size_bytes,
        };

        match stored_hash {
            None => new_files.push(entry),
            Some(h) if h != entry.content_hash => changed_files.push(entry),
            Some(_) => unchanged_files.push(entry),
        }
    }

    // Find removed files
    let mut removed_files = Vec::new();
    let mut stmt = db.prepare("SELECT file_path, content_hash, file_type, language, size_bytes FROM file_manifest")?;
    let rows: Vec<(String, String, String, Option<String>, u64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (fp, hash, ft, lang, size) in rows {
        if !seen_paths.contains(&fp) {
            removed_files.push(FileEntry {
                path: PathBuf::from(&fp),
                file_type: FileType::from_str(&ft).unwrap_or(FileType::Code),
                language: lang,
                content_hash: hash,
                size_bytes: size,
            });
        }
    }

    Ok(DetectResult {
        new: new_files,
        changed: changed_files,
        unchanged: unchanged_files,
        removed: removed_files,
    })
}

pub fn update_manifest(result: &DetectResult, db: &Connection) -> graphify_core::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let all_entries: Vec<&FileEntry> = result.new.iter().chain(result.changed.iter()).chain(result.unchanged.iter()).collect();
    for entry in &all_entries {
        db.execute(
            "INSERT OR REPLACE INTO file_manifest (file_path, content_hash, file_type, language, last_seen_at, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                entry.path.to_string_lossy().to_string().replace('\\', "/"),
                entry.content_hash,
                entry.file_type.as_str(),
                entry.language,
                now,
                entry.size_bytes,
            ],
        )?;
    }
    for entry in &result.removed {
        db.execute("DELETE FROM file_manifest WHERE file_path = ?1", rusqlite::params![entry.path.to_string_lossy().to_string()])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use graphify_core::db::open_db_in_memory;

    #[test]
    fn classify_known_extensions() {
        assert_eq!(classify_file(Path::new("foo.py")), Some(FileType::Code));
        assert_eq!(classify_file(Path::new("foo.rs")), Some(FileType::Code));
        assert_eq!(classify_file(Path::new("foo.md")), Some(FileType::Document));
        assert_eq!(classify_file(Path::new("foo.pdf")), Some(FileType::Paper));
        assert_eq!(classify_file(Path::new("foo.png")), Some(FileType::Image));
        assert_eq!(classify_file(Path::new("foo.mp4")), Some(FileType::Video));
        assert_eq!(classify_file(Path::new("foo.xyz")), None);
        assert_eq!(classify_file(Path::new("Makefile")), None);
    }

    #[test]
    fn language_for_ext() {
        assert_eq!(language_for_extension(".py"), Some("Python"));
        assert_eq!(language_for_extension(".rs"), Some("Rust"));
        assert_eq!(language_for_extension(".xyz"), None);
    }

    #[test]
    fn detect_new_files_in_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "def hello(): pass\n").unwrap();
        fs::write(dir.path().join("readme.md"), "# Hello\n").unwrap();

        let db = open_db_in_memory().unwrap();
        let result = detect(dir.path(), &db).unwrap();

        assert_eq!(result.new.len(), 2);
        assert_eq!(result.changed.len(), 0);
        assert_eq!(result.removed.len(), 0);
        assert!(result.new.iter().any(|f| f.path.to_string_lossy().contains("main.py")));
        assert!(result.new.iter().any(|f| f.language.as_deref() == Some("Python")));
    }

    #[test]
    fn detect_changed_files_after_update() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "def hello(): pass\n").unwrap();

        let db = open_db_in_memory().unwrap();
        let result = detect(dir.path(), &db).unwrap();
        update_manifest(&result, &db).unwrap();

        fs::write(dir.path().join("main.py"), "def goodbye(): pass\n").unwrap();
        let result2 = detect(dir.path(), &db).unwrap();

        assert_eq!(result2.new.len(), 0);
        assert_eq!(result2.changed.len(), 1);
    }

    #[test]
    fn detect_removed_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.py"), "a\n").unwrap();
        fs::write(dir.path().join("b.py"), "b\n").unwrap();

        let db = open_db_in_memory().unwrap();
        let result = detect(dir.path(), &db).unwrap();
        update_manifest(&result, &db).unwrap();

        fs::remove_file(dir.path().join("b.py")).unwrap();
        let result2 = detect(dir.path(), &db).unwrap();

        assert_eq!(result2.removed.len(), 1);
        assert!(result2.removed[0].path.to_string_lossy().contains("b.py"));
    }
}
