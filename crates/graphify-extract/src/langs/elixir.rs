use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    static CONFIG: LanguageConfig = LanguageConfig {
        name: "Elixir",
        extensions: &[".ex", ".exs"],
        language_fn: || tree_sitter_elixir::LANGUAGE.into(),
        // Elixir uses `call` nodes for defmodule, def, defp, import, use, alias, etc.
        // The tree-sitter grammar doesn't distinguish them by node kind, so we only
        // extract class_types (defmodule via call) and function_types (def/defp via call).
        // import_types is left empty because `call` is too broad — every function call
        // would be treated as an import edge.
        class_types: &["call"], // defmodule (identified by first child being :defmodule)
        function_types: &["call"], // def, defp, defmacro
        import_types: &[],      // Cannot distinguish import calls from other calls
        call_type: "call",
        name_field: "name",
        body_field: None,
        body_fallback_types: &["do_block", "stab_clause"],
    };
    &CONFIG
}
