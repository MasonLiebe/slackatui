#![allow(dead_code)]

mod auth;
mod config;
mod parse;
mod service;
mod slack;
mod tui;
mod types;

use auth::{load_tokens, store_tokens, StoreType};
use config::Config;
use service::SlackService;
use slack::SlackClient;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let config_path = config::default_config_path();
    let cfg = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    // Handle `auth` subcommand
    if args.len() > 1 && args[1] == "auth" {
        run_auth(&cfg);
        return;
    }

    // Load stored token
    let store_type = match StoreType::from_str(&cfg.auth.token_store) {
        Ok(st) => st,
        Err(e) => {
            eprintln!("Invalid token_store config: {}", e);
            std::process::exit(1);
        }
    };

    let tokens = match load_tokens(&cfg.auth.team_id, &store_type) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("No stored tokens found. Run `slackatui auth` first to authenticate.");
            std::process::exit(1);
        }
    };

    // Pick the token to use based on preference
    let token = match cfg.auth.token_preference.as_str() {
        "bot" if !tokens.bot_token.is_empty() => &tokens.bot_token,
        _ if !tokens.user_token.is_empty() => &tokens.user_token,
        _ if !tokens.bot_token.is_empty() => &tokens.bot_token,
        _ => {
            eprintln!("No valid token found. Run `slackatui auth` to re-authenticate.");
            std::process::exit(1);
        }
    };

    let client = SlackClient::new(token);
    let svc = SlackService::new(client, cfg.emoji);

    // Run the TUI with the tokio runtime
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        if let Err(e) = tui::run_async(cfg, svc).await {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    });
}

fn run_auth(cfg: &Config) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let oauth_cfg = auth::OAuthConfig {
            client_id: cfg.auth.client_id.clone(),
            client_secret: String::new(),
            redirect_uri: cfg.auth.redirect_uri.clone(),
            scopes: Vec::new(),
            user_scopes: auth::DEFAULT_USER_SCOPES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            port: 8888,
        };

        if oauth_cfg.client_id.is_empty() {
            eprintln!("Error: auth.client_id is not set in config.");
            eprintln!("Set it in: {}", config::default_config_path().display());
            std::process::exit(1);
        }

        match auth::run_oauth_flow(&mut oauth_cfg.clone()).await {
            Ok(token_resp) => {
                let stored = auth::parse_token_response(&token_resp);
                let store_type = StoreType::from_str(&cfg.auth.token_store)
                    .unwrap_or(StoreType::Keychain);

                if let Err(e) = store_tokens(&stored, &store_type) {
                    eprintln!("Failed to store tokens: {}", e);
                    std::process::exit(1);
                }

                println!("Authentication successful!");
                println!("Team: {}", stored.team_name);
                println!("Tokens stored in: {:?}", store_type);
            }
            Err(e) => {
                eprintln!("OAuth flow failed: {:?}", e);
                std::process::exit(1);
            }
        }
    });
}
