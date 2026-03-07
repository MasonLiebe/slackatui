mod auth;
mod config;
mod parse;
mod service;
mod slack;
mod tui;
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

    if let Err(e) = tui::run(cfg) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
