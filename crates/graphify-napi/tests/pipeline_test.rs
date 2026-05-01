use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
}

/// Copy a fixture directory into a temp directory so tests can run in parallel
/// without interfering with each other's .graphify state.
fn copy_fixture_to_temp(subdir: &str) -> tempfile::TempDir {
    let src = fixtures_dir().join(subdir);
    let tmp = tempfile::tempdir().unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let entry = entry.unwrap();
        let dst = tmp.path().join(entry.file_name());
        std::fs::copy(entry.path(), dst).unwrap();
    }
    tmp
}

#[test]
fn full_pipeline_on_python_fixture() {
    let tmp = copy_fixture_to_temp("python");
    let root = tmp.path();
    let result = graphify_napi::pipeline::run_pipeline(root).unwrap();

    assert!(
        result.build_result.nodes_added > 0,
        "should extract nodes from Python fixture"
    );
    assert!(
        result.build_result.edges_added > 0,
        "should extract edges from Python fixture"
    );
    assert!(
        !result.analysis.god_nodes.is_empty(),
        "should find god nodes"
    );
    assert!(result.report.contains("# Graph Report"));

    assert!(root.join(".graphify/db.sqlite").exists());
    assert!(root.join(".graphify/graph_report.md").exists());
    assert!(root.join(".graphify/graph.json").exists());
}

#[test]
fn full_pipeline_on_rust_fixture() {
    let tmp = copy_fixture_to_temp("rust");
    let root = tmp.path();
    let result = graphify_napi::pipeline::run_pipeline(root).unwrap();
    assert!(result.build_result.nodes_added > 0);
}

#[test]
fn full_pipeline_on_javascript_fixture() {
    let tmp = copy_fixture_to_temp("javascript");
    let root = tmp.path();
    let result = graphify_napi::pipeline::run_pipeline(root).unwrap();
    assert!(result.build_result.nodes_added > 0);
}

#[test]
fn incremental_update_adds_no_duplicate_nodes() {
    let tmp = copy_fixture_to_temp("python");
    let root = tmp.path();
    let r1 = graphify_napi::pipeline::run_pipeline(root).unwrap();
    assert!(r1.build_result.nodes_added > 0);

    let r2 = graphify_napi::pipeline::run_pipeline(root).unwrap();
    assert_eq!(
        r2.build_result.nodes_added, 0,
        "second run on unchanged files should add 0 nodes"
    );
}

#[test]
fn export_json_is_valid() {
    let tmp = copy_fixture_to_temp("typescript");
    let root = tmp.path();
    let _ = graphify_napi::pipeline::run_pipeline(root).unwrap();

    let json_str = std::fs::read_to_string(root.join(".graphify/graph.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed["nodes"].is_array());
    assert!(parsed["edges"].is_array());
    assert!(parsed["nodes"].as_array().unwrap().len() > 0);
}
