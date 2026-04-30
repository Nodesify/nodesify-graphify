pub mod config;
pub mod python;
pub mod javascript;
pub mod typescript;
pub mod rust;
pub mod go;
pub mod java;
pub mod c;

pub use config::LanguageConfig;

pub fn get_language_for_extension(ext: &str) -> Option<&'static LanguageConfig> {
    let ext_with_dot = if ext.starts_with('.') {
        ext.to_lowercase()
    } else {
        format!(".{}", ext).to_lowercase()
    };
    all_languages()
        .iter()
        .find(|cfg| cfg.extensions.contains(&ext_with_dot.as_str()))
        .copied()
}

pub fn all_languages() -> Vec<&'static LanguageConfig> {
    vec![
        python::config(),
        javascript::config(),
        typescript::config(),
        rust::config(),
        go::config(),
        java::config(),
        c::config(),
        c::cpp_config(),
    ]
}
