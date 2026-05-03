// Kotlin tree-sitter grammar: tree-sitter-kotlin 0.3 depends on tree-sitter 0.20,
// incompatible with our tree-sitter 0.25. The dependency has been removed from
// Cargo.toml. To re-enable:
//   1. Add `tree-sitter-kotlin` back to workspace and extract crate Cargo.toml
//      once a tree-sitter 0.25+ compatible version is released.
//   2. Replace `kotlin_language()` with `tree_sitter_kotlin::LANGUAGE.into()`.
//   3. Uncomment `kotlin::config()` in `all_languages()` in mod.rs.

use super::config::LanguageConfig;

// Stub: returns Python grammar as placeholder. This config is NOT registered in
// all_languages() and will never be used for parsing in its current state.
fn kotlin_language() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

pub fn config() -> &'static LanguageConfig {
    static CONFIG: LanguageConfig = LanguageConfig {
        name: "Kotlin",
        extensions: &[".kt", ".kts"],
        language_fn: kotlin_language,
        class_types: &[
            "class_declaration",
            "object_declaration",
            "interface_declaration",
            "enum_declaration",
        ],
        function_types: &["function_declaration"],
        import_types: &["import_header"],
        call_type: "call_expression",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &["function_body"],
    };
    &CONFIG
}
