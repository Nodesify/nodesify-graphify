// Kotlin tree-sitter grammar (v0.3) depends on tree-sitter 0.20 which is
// incompatible with our tree-sitter 0.25. This config is defined for future use
// but excluded from the active language list in mod.rs until a compatible
// tree-sitter-kotlin crate is released.

use super::config::LanguageConfig;

// Stub: returns a no-op language. This config is NOT registered in all_languages().
// Replace with `tree_sitter_kotlin::LANGUAGE.into()` once the crate supports tree-sitter 0.25+.
fn kotlin_language() -> tree_sitter::Language {
    // Using Python grammar as placeholder — this config is never used for parsing.
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
