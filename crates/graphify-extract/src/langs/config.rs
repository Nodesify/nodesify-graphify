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
}
