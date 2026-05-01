use std::path::{Path, PathBuf};

/// Normalize a path to forward-slash representation for DB storage.
/// Converts all backslashes to forward slashes on any platform.
pub fn normalize(p: &Path) -> String {
    p.to_string_lossy().to_string().replace('\\', "/")
}

/// Return the `.graphify` directory path under the given root.
/// Creates the directory if it does not exist.
pub fn graphify_dir(root: &Path) -> std::io::Result<PathBuf> {
    let dir = root.join(".graphify");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Return the SQLite database path: `root/.graphify/db.sqlite`.
/// Creates the `.graphify` directory if it does not exist.
pub fn db_path(root: &Path) -> std::io::Result<PathBuf> {
    let dir = graphify_dir(root)?;
    Ok(dir.join("db.sqlite"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn normalize_unix_path_unchanged() {
        assert_eq!(normalize(Path::new("src/main.rs")), "src/main.rs");
    }

    #[test]
    fn normalize_converts_backslashes() {
        assert_eq!(normalize(Path::new("src\\main.rs")), "src/main.rs");
    }

    #[test]
    fn normalize_mixed_separators() {
        assert_eq!(normalize(Path::new("src\\lib/mod.rs")), "src/lib/mod.rs");
    }

    #[test]
    fn normalize_already_normalized() {
        assert_eq!(normalize(Path::new("a/b/c.py")), "a/b/c.py");
    }

    #[test]
    fn graphify_dir_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let gf = graphify_dir(dir.path()).unwrap();
        assert!(gf.exists());
        assert!(gf.to_string_lossy().contains(".graphify"));
    }

    #[test]
    fn db_path_is_inside_graphify_dir() {
        let dir = tempfile::tempdir().unwrap();
        let db = db_path(dir.path()).unwrap();
        assert!(db.to_string_lossy().contains(".graphify"));
        assert!(db.to_string_lossy().contains("db.sqlite"));
    }
}
