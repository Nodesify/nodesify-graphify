use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    static CONFIG: LanguageConfig = LanguageConfig {
        name: "PHP",
        extensions: &[".php"],
        language_fn: || tree_sitter_php::LANGUAGE_PHP.into(),
        class_types: &[
            "class_declaration",
            "interface_declaration",
            "trait_declaration",
            "enum_declaration",
        ],
        function_types: &[
            "function_definition",
            "method_declaration",
            "declaration_list",
        ],
        import_types: &["namespace_use_declaration", "namespace_definition"],
        call_type: "function_call_expression",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &["compound_statement", "declaration_list"],
        class_call_names: &[],
        function_call_names: &[],
        import_call_names: &[],
    };
    &CONFIG
}
