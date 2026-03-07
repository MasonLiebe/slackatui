mod auth;
mod config;
mod types;

use config::Config;

fn main() {
    let config_path = config::default_config_path();
    let cfg = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    println!("slackatui v0.1.0");
    println!("Config loaded from: {}", config_path.display());
    println!("  sidebar_width: {}", cfg.sidebar_width);
    println!("  emoji: {}", cfg.emoji);
    println!("  token_store: {}", cfg.auth.token_store);
}
