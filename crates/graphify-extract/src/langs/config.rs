use tree_sitter::Language;

pub struct LanguageConfig {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub language_fn: fn() -> Language,
    pub class_types: &'static [&'static str],
    pub function_types: &'static [&'static str],
    pub import_types: &'static [&'static str],
    pub call_type: &'static str,
    pub name_field: &'static str,
    pub body_field: Option<&'static str>,
    pub body_fallback_types: &'static [&'static str],
    /// When non-empty, only classify a node matching `class_types` as a class
    /// if the first child's text is in this list. Used for languages like Elixir
    /// where the grammar uses a single node kind for multiple constructs.
    pub class_call_names: &'static [&'static str],
    pub function_call_names: &'static [&'static str],
    pub import_call_names: &'static [&'static str],
}
