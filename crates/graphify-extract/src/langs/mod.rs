pub mod config;
pub mod python;
pub mod javascript;
pub mod typescript;
pub mod rust;
pub mod go;
pub mod java;
pub mod c;
pub mod ruby;
pub mod swift;
pub mod kotlin;
pub mod scala;
pub mod php;
pub mod c_sharp;
pub mod lua;
pub mod haskell;
pub mod elixir;
pub mod shell;
pub mod dart;
pub mod zig;
pub mod css;

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
        ruby::config(),
        swift::config(),
        // kotlin::config() — disabled: tree-sitter-kotlin 0.3 depends on tree-sitter 0.20,
        // incompatible with our tree-sitter 0.25. Re-enable once a compatible version is released.
        scala::config(),
        php::config(),
        c_sharp::config(),
        lua::config(),
        haskell::config(),
        elixir::config(),
        shell::config(),
        dart::config(),
        zig::config(),
        css::config(),
    ]
}
