use std::io;

struct Config {
    name: String,
}

impl Config {
    fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}

fn main() {
    let config = Config::new("test");
    println!("{}", config.name);
}
