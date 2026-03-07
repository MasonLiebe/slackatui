use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

pub const NOTIFY_ALL: &str = "all";
pub const NOTIFY_MENTION: &str = "mention";

/// Returns the default config file path: ~/.config/slackatui/config
pub fn default_config_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    config_dir.join("slackatui").join("config")
}

/// OAuth v2 and token storage settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub redirect_uri: String,
    #[serde(default)]
    pub token_store: String,
    #[serde(default)]
    pub token_preference: String,
    #[serde(default)]
    pub team_id: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: String::new(),
            token_store: "keychain".to_string(),
            token_preference: "user".to_string(),
            team_id: String::new(),
        }
    }
}

/// Theme configuration for views, channels, and messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default = "ViewTheme::default")]
    pub view: ViewTheme,
    #[serde(default)]
    pub channel: ChannelTheme,
    #[serde(default)]
    pub message: MessageTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            view: ViewTheme::default(),
            channel: ChannelTheme::default(),
            message: MessageTheme::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewTheme {
    #[serde(default = "default_white")]
    pub fg: String,
    #[serde(default = "default_bg")]
    pub bg: String,
    #[serde(default = "default_white")]
    pub border_fg: String,
    #[serde(default)]
    pub border_bg: String,
    #[serde(default = "default_label_fg")]
    pub label_fg: String,
    #[serde(default)]
    pub label_bg: String,
}

impl Default for ViewTheme {
    fn default() -> Self {
        Self {
            fg: "white".to_string(),
            bg: "default".to_string(),
            border_fg: "white".to_string(),
            border_bg: String::new(),
            label_fg: "green,bold".to_string(),
            label_bg: String::new(),
        }
    }
}

fn default_white() -> String {
    "white".to_string()
}
fn default_bg() -> String {
    "default".to_string()
}
fn default_label_fg() -> String {
    "green,bold".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelTheme {
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageTheme {
    #[serde(default)]
    pub time: String,
    #[serde(default = "default_time_format")]
    pub time_format: String,
    #[serde(default = "default_thread_style")]
    pub thread: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub text: String,
}

impl Default for MessageTheme {
    fn default() -> Self {
        Self {
            time: String::new(),
            time_format: "15:04".to_string(),
            thread: "fg-bold".to_string(),
            name: String::new(),
            text: String::new(),
        }
    }
}

fn default_time_format() -> String {
    "15:04".to_string()
}
fn default_thread_style() -> String {
    "fg-bold".to_string()
}

/// A key mapping is a map from key string (e.g. "j", "C-b", "<enter>") to action name.
pub type KeyMapping = HashMap<String, String>;

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub notify: String,
    #[serde(default)]
    pub emoji: bool,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,
    #[serde(skip)]
    pub main_width: u16,
    #[serde(default = "default_threads_width")]
    pub threads_width: u16,
    #[serde(default = "default_keymap")]
    pub key_map: HashMap<String, KeyMapping>,
    #[serde(default)]
    pub theme: Theme,
}

fn default_sidebar_width() -> u16 {
    1
}
fn default_threads_width() -> u16 {
    1
}

fn default_keymap() -> HashMap<String, KeyMapping> {
    let mut key_map = HashMap::new();

    let mut command = KeyMapping::new();
    command.insert("i".into(), "mode-insert".into());
    command.insert("/".into(), "mode-search".into());
    command.insert("k".into(), "channel-up".into());
    command.insert("j".into(), "channel-down".into());
    command.insert("g".into(), "channel-top".into());
    command.insert("G".into(), "channel-bottom".into());
    command.insert("K".into(), "thread-up".into());
    command.insert("J".into(), "thread-down".into());
    command.insert("<previous>".into(), "chat-up".into());
    command.insert("C-b".into(), "chat-up".into());
    command.insert("C-u".into(), "chat-up".into());
    command.insert("<next>".into(), "chat-down".into());
    command.insert("C-f".into(), "chat-down".into());
    command.insert("C-d".into(), "chat-down".into());
    command.insert("n".into(), "channel-search-next".into());
    command.insert("N".into(), "channel-search-prev".into());
    command.insert("l".into(), "focus-right".into());
    command.insert("h".into(), "focus-left".into());
    command.insert("<enter>".into(), "select".into());
    command.insert("'".into(), "open-thread".into());
    command.insert("r".into(), "reply".into());
    command.insert("e".into(), "mode-react".into());
    command.insert("o".into(), "open-file".into());
    command.insert("u".into(), "upload-file".into());
    command.insert("p".into(), "toggle-presence".into());
    command.insert("q".into(), "quit".into());
    command.insert("<f1>".into(), "help".into());
    key_map.insert("command".into(), command);

    let mut insert = KeyMapping::new();
    insert.insert("<left>".into(), "cursor-left".into());
    insert.insert("<right>".into(), "cursor-right".into());
    insert.insert("<enter>".into(), "send".into());
    insert.insert("<s-enter>".into(), "newline".into());
    insert.insert("<escape>".into(), "mode-command".into());
    insert.insert("<backspace>".into(), "backspace".into());
    insert.insert("C-8".into(), "backspace".into());
    insert.insert("<delete>".into(), "delete".into());
    insert.insert("<space>".into(), "space".into());
    insert.insert("<tab>".into(), "indent".into());
    insert.insert("<s-tab>".into(), "dedent".into());
    insert.insert("C-b".into(), "toggle-bold".into());
    insert.insert("C-i".into(), "toggle-italic".into());
    insert.insert("C-u".into(), "toggle-underline".into());
    key_map.insert("insert".into(), insert);

    let mut search = KeyMapping::new();
    search.insert("<left>".into(), "cursor-left".into());
    search.insert("<right>".into(), "cursor-right".into());
    search.insert("<escape>".into(), "clear-input".into());
    search.insert("<enter>".into(), "clear-input".into());
    search.insert("<backspace>".into(), "backspace".into());
    search.insert("C-8".into(), "backspace".into());
    search.insert("<delete>".into(), "delete".into());
    search.insert("<space>".into(), "space".into());
    key_map.insert("search".into(), search);

    key_map
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auth: AuthConfig::default(),
            notify: String::new(),
            emoji: false,
            sidebar_width: 1,
            main_width: 11,
            threads_width: 1,
            key_map: default_keymap(),
            theme: Theme::default(),
        }
    }
}

impl Config {
    /// Load config from the given path. If the file doesn't exist, create a
    /// default config file and return defaults.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                let mut cfg: Config = serde_json::from_str(&contents)
                    .map_err(|e| ConfigError::InvalidJson(e.to_string()))?;
                cfg.validate()?;
                cfg.main_width = 12 - cfg.sidebar_width;
                Ok(cfg)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let cfg = Config::create_default_file(path)?;
                Ok(cfg)
            }
            Err(e) => Err(ConfigError::Io(e.to_string())),
        }
    }

    /// Validate config values.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.sidebar_width < 1 || self.sidebar_width > 11 {
            return Err(ConfigError::Validation(
                "sidebar_width must be between 1 and 11".to_string(),
            ));
        }

        match self.notify.as_str() {
            "" | NOTIFY_ALL | NOTIFY_MENTION => {}
            other => {
                return Err(ConfigError::Validation(format!(
                    "unsupported notify setting: {}",
                    other
                )));
            }
        }

        Ok(())
    }

    /// Create a default config file at the given path and return the default config.
    fn create_default_file(path: &Path) -> Result<Self, ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e.to_string()))?;
        }

        let cfg = Config::default();
        let json = serde_json::to_string_pretty(&cfg)
            .map_err(|e| ConfigError::Io(e.to_string()))?;
        fs::write(path, json).map_err(|e| ConfigError::Io(e.to_string()))?;

        Ok(cfg)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(String),
    InvalidJson(String),
    Validation(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(msg) => write!(f, "config I/O error: {}", msg),
            ConfigError::InvalidJson(msg) => write!(f, "config is not valid JSON: {}", msg),
            ConfigError::Validation(msg) => write!(f, "config validation error: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

// ---- Interactive config wizard ----

/// Read a line from stdin, trimmed. Returns empty string on EOF.
fn prompt(label: &str) -> String {
    print!("{}", label);
    let _ = io::stdout().flush();
    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
    buf.trim().to_string()
}

/// Prompt with a default value shown in brackets. Empty input returns the default.
fn prompt_default(label: &str, default: &str) -> String {
    let input = prompt(&format!("{} [{}]: ", label, default));
    if input.is_empty() { default.to_string() } else { input }
}

/// Prompt for a yes/no question. Returns bool.
fn prompt_yn(label: &str, default: bool) -> bool {
    let hint = if default { "Y/n" } else { "y/N" };
    let input = prompt(&format!("{} [{}]: ", label, hint));
    match input.to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
}

/// Prompt user to pick from a numbered list. Returns the chosen value.
fn prompt_choice(label: &str, options: &[(&str, &str)], default_idx: usize) -> String {
    println!("\n  {}", label);
    for (i, (value, desc)) in options.iter().enumerate() {
        let marker = if i == default_idx { " (default)" } else { "" };
        println!("    {}. {} - {}{}", i + 1, value, desc, marker);
    }
    let input = prompt(&format!("  Choose [{}]: ", default_idx + 1));
    if let Ok(n) = input.parse::<usize>() {
        if n >= 1 && n <= options.len() {
            return options[n - 1].0.to_string();
        }
    }
    options[default_idx].0.to_string()
}

/// Run the interactive configuration wizard.
/// Loads existing config, walks the user through each setting, preserves
/// key_map untouched, and writes the result.
pub fn run_config_wizard() {
    let path = default_config_path();

    // Load existing config (or defaults)
    let existing = Config::load(&path).unwrap_or_default();

    println!();
    println!("  \x1b[1;36mslackatui configuration\x1b[0m");
    println!("  ─────────────────────");
    println!();
    println!("  Walk through each setting below. Press Enter to keep the");
    println!("  current value shown in brackets.");
    println!();

    // ── Slack App credentials ──
    println!("  \x1b[1;33m▸ Slack App Credentials\x1b[0m");
    println!("    These come from your Slack App at https://api.slack.com/apps");
    println!("    under \"Basic Information\" → \"App Credentials\".");
    println!();

    let client_id = prompt_default(
        "    Client ID",
        if existing.auth.client_id.is_empty() { "<not set>" } else { &existing.auth.client_id },
    );
    let client_id = if client_id == "<not set>" { String::new() } else { client_id };

    let client_secret = prompt_default(
        "    Client Secret",
        if existing.auth.client_secret.is_empty() { "<not set>" } else { &existing.auth.client_secret },
    );
    let client_secret = if client_secret == "<not set>" { String::new() } else { client_secret };

    let redirect_uri = prompt_default(
        "    Redirect URI",
        if existing.auth.redirect_uri.is_empty() { "https://localhost:8888/auth/callback" } else { &existing.auth.redirect_uri },
    );

    // ── Token storage ──
    println!();
    println!("  \x1b[1;33m▸ Token Storage\x1b[0m");
    println!("    Where to store your Slack OAuth tokens after authentication.");
    println!();

    let token_store_default = match existing.auth.token_store.as_str() {
        "file" => 1,
        _ => 0,
    };
    let token_store = prompt_choice(
        "Token storage backend:",
        &[
            ("keychain", "macOS Keychain (secure, recommended)"),
            ("file", "plain JSON file (~/.config/slackatui/tokens)"),
        ],
        token_store_default,
    );

    let token_pref_default = match existing.auth.token_preference.as_str() {
        "bot" => 1,
        _ => 0,
    };
    let token_preference = prompt_choice(
        "Token preference (which token to use when both are available):",
        &[
            ("user", "user token (full user access, recommended)"),
            ("bot", "bot token (limited to bot scopes)"),
        ],
        token_pref_default,
    );

    let team_id = prompt_default(
        "\n    Team ID (optional, for multi-workspace)",
        if existing.auth.team_id.is_empty() { "<auto>" } else { &existing.auth.team_id },
    );
    let team_id = if team_id == "<auto>" { String::new() } else { team_id };

    // ── Notifications ──
    println!();
    println!("  \x1b[1;33m▸ Notifications\x1b[0m");
    println!("    OS desktop notifications for incoming messages.");
    println!();

    let notify_default = match existing.notify.as_str() {
        "all" => 1,
        "mention" => 2,
        _ => 0,
    };
    let notify = prompt_choice(
        "When should notifications appear?",
        &[
            ("", "off - no desktop notifications"),
            ("all", "all - notify on every incoming message"),
            ("mention", "mention - only DMs and @mentions"),
        ],
        notify_default,
    );

    // ── Emoji ──
    println!();
    println!("  \x1b[1;33m▸ Emoji\x1b[0m");
    println!("    Render :emoji_codes: as Unicode emoji characters in messages.");
    println!();

    let emoji = prompt_yn("    Enable emoji rendering?", existing.emoji);

    // ── Layout ──
    println!();
    println!("  \x1b[1;33m▸ Layout\x1b[0m");
    println!("    Control the proportional widths of UI panels.");
    println!("    Total width is divided into 12 columns.");
    println!();

    let sidebar_str = prompt_default(
        "    Sidebar width (1-5, in 12ths of screen)",
        &existing.sidebar_width.to_string(),
    );
    let sidebar_width: u16 = sidebar_str.parse().unwrap_or(existing.sidebar_width).clamp(1, 5);

    let threads_str = prompt_default(
        "    Threads panel width (1-5, in 12ths of screen)",
        &existing.threads_width.to_string(),
    );
    let threads_width: u16 = threads_str.parse().unwrap_or(existing.threads_width).clamp(1, 5);

    // ── Theme ──
    println!();
    println!("  \x1b[1;33m▸ Theme\x1b[0m");
    println!("    Customize colors. Use color names (red, green, blue, white, default)");
    println!("    or add modifiers (e.g. \"green,bold\"). Leave blank for defaults.");
    println!();

    let view_fg = prompt_default("    View foreground", &existing.theme.view.fg);
    let view_bg = prompt_default("    View background", &existing.theme.view.bg);
    let border_fg = prompt_default("    Border foreground", &existing.theme.view.border_fg);
    let label_fg = prompt_default("    Label style", &existing.theme.view.label_fg);

    // ── Message display ──
    println!();
    println!("  \x1b[1;33m▸ Message Display\x1b[0m");
    println!("    How messages are formatted in the chat view.");
    println!();

    let time_format_default = match existing.theme.message.time_format.as_str() {
        "15:04:05" => 1,
        "3:04 PM" => 2,
        _ => 0,
    };
    let time_format = prompt_choice(
        "Timestamp format:",
        &[
            ("15:04", "24-hour short (15:04)"),
            ("15:04:05", "24-hour with seconds (15:04:05)"),
            ("3:04 PM", "12-hour (3:04 PM)"),
        ],
        time_format_default,
    );

    // ── Presence ──
    println!();
    println!("  \x1b[1;33m▸ Presence\x1b[0m");
    println!("    slackatui automatically sets you as \"active\" on launch and shows");
    println!("    online/away indicators for DM contacts in the sidebar.");
    println!("    Press \x1b[1mp\x1b[0m in command mode to toggle your status at any time.");
    println!();

    // ── Summary & save ──
    println!();
    println!("  \x1b[1;33m▸ Summary\x1b[0m");
    println!();
    println!("    Notifications:   {}", if notify.is_empty() { "off" } else { &notify });
    println!("    Emoji:           {}", if emoji { "on" } else { "off" });
    println!("    Sidebar width:   {}/12", sidebar_width);
    println!("    Threads width:   {}/12", threads_width);
    println!("    Time format:     {}", time_format);
    println!("    Token storage:   {}", token_store);
    println!();

    if !prompt_yn("    Save this configuration?", true) {
        println!("\n  Configuration not saved.");
        return;
    }

    // Build config — preserve existing key_map untouched
    let cfg = Config {
        auth: AuthConfig {
            client_id,
            client_secret,
            redirect_uri,
            token_store,
            token_preference,
            team_id,
        },
        notify,
        emoji,
        sidebar_width,
        main_width: 12 - sidebar_width,
        threads_width,
        key_map: existing.key_map,
        theme: Theme {
            view: ViewTheme {
                fg: view_fg,
                bg: view_bg,
                border_fg,
                border_bg: existing.theme.view.border_bg,
                label_fg,
                label_bg: existing.theme.view.label_bg,
            },
            channel: existing.theme.channel,
            message: MessageTheme {
                time: existing.theme.message.time,
                time_format,
                thread: existing.theme.message.thread,
                name: existing.theme.message.name,
                text: existing.theme.message.text,
            },
        },
    };

    // Write
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    match serde_json::to_string_pretty(&cfg) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, &json) {
                eprintln!("\n  \x1b[1;31mError saving config:\x1b[0m {}", e);
                return;
            }
        }
        Err(e) => {
            eprintln!("\n  \x1b[1;31mError serializing config:\x1b[0m {}", e);
            return;
        }
    }

    println!();
    println!("  \x1b[1;32m✓ Configuration saved to:\x1b[0m");
    println!("    {}", path.display());
    println!();
    println!("  You can edit this file directly at any time, or re-run");
    println!("  \x1b[1mslackatui config\x1b[0m to go through this wizard again.");
    println!();
    println!("  \x1b[2mNote: keybindings are preserved and can be edited");
    println!("  directly in the config JSON under \"key_map\".\x1b[0m");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.sidebar_width, 1);
        assert_eq!(cfg.main_width, 11);
        assert_eq!(cfg.threads_width, 1);
        assert!(!cfg.emoji);
        assert!(cfg.notify.is_empty());
        assert_eq!(cfg.auth.token_store, "keychain");
        assert_eq!(cfg.auth.token_preference, "user");
    }

    #[test]
    fn test_default_keymap_has_all_modes() {
        let cfg = Config::default();
        assert!(cfg.key_map.contains_key("command"));
        assert!(cfg.key_map.contains_key("insert"));
        assert!(cfg.key_map.contains_key("search"));
    }

    #[test]
    fn test_default_keymap_command_bindings() {
        let cfg = Config::default();
        let cmd = &cfg.key_map["command"];
        assert_eq!(cmd.get("i").unwrap(), "mode-insert");
        assert_eq!(cmd.get("q").unwrap(), "quit");
        assert_eq!(cmd.get("j").unwrap(), "channel-down");
        assert_eq!(cmd.get("k").unwrap(), "channel-up");
        assert_eq!(cmd.get("<f1>").unwrap(), "help");
    }

    #[test]
    fn test_default_keymap_insert_bindings() {
        let cfg = Config::default();
        let ins = &cfg.key_map["insert"];
        assert_eq!(ins.get("<enter>").unwrap(), "send");
        assert_eq!(ins.get("<escape>").unwrap(), "mode-command");
        assert_eq!(ins.get("<backspace>").unwrap(), "backspace");
    }

    #[test]
    fn test_default_theme() {
        let cfg = Config::default();
        assert_eq!(cfg.theme.view.fg, "white");
        assert_eq!(cfg.theme.view.bg, "default");
        assert_eq!(cfg.theme.view.label_fg, "green,bold");
        assert_eq!(cfg.theme.message.time_format, "15:04");
        assert_eq!(cfg.theme.message.thread, "fg-bold");
    }

    #[test]
    fn test_load_from_json_string() {
        let json = r#"{
            "auth": {
                "client_id": "123.456",
                "token_store": "file"
            },
            "notify": "mention",
            "emoji": true,
            "sidebar_width": 3,
            "threads_width": 2,
            "theme": {
                "view": { "fg": "red" }
            }
        }"#;

        let cfg: Config = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.auth.client_id, "123.456");
        assert_eq!(cfg.auth.token_store, "file");
        assert_eq!(cfg.notify, "mention");
        assert!(cfg.emoji);
        assert_eq!(cfg.sidebar_width, 3);
        assert_eq!(cfg.threads_width, 2);
        assert_eq!(cfg.theme.view.fg, "red");
        // Defaults should still apply to unset fields
        assert_eq!(cfg.theme.view.border_fg, "white");
        assert_eq!(cfg.theme.message.time_format, "15:04");
    }

    #[test]
    fn test_load_creates_default_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slackatui").join("config");

        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.sidebar_width, 1);
        assert_eq!(cfg.main_width, 11);
        assert!(path.exists());

        // File should be valid JSON that round-trips
        let contents = fs::read_to_string(&path).unwrap();
        let reloaded: Config = serde_json::from_str(&contents).unwrap();
        assert_eq!(reloaded.sidebar_width, cfg.sidebar_width);
    }

    #[test]
    fn test_load_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");

        let json = r#"{"sidebar_width": 4, "emoji": true}"#;
        fs::write(&path, json).unwrap();

        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.sidebar_width, 4);
        assert_eq!(cfg.main_width, 8);
        assert!(cfg.emoji);
    }

    #[test]
    fn test_load_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");

        fs::write(&path, "not json at all {{{").unwrap();

        let result = Config::load(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::InvalidJson(_)));
    }

    #[test]
    fn test_validate_sidebar_width_too_large() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");

        fs::write(&path, r#"{"sidebar_width": 12}"#).unwrap();

        let result = Config::load(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
    }

    #[test]
    fn test_validate_sidebar_width_zero() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");

        fs::write(&path, r#"{"sidebar_width": 0}"#).unwrap();

        let result = Config::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_bad_notify() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");

        fs::write(&path, r#"{"notify": "invalid"}"#).unwrap();

        let result = Config::load(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
    }

    #[test]
    fn test_auth_config_defaults() {
        let auth = AuthConfig::default();
        assert!(auth.client_id.is_empty());
        assert_eq!(auth.token_store, "keychain");
        assert_eq!(auth.token_preference, "user");
        assert!(auth.team_id.is_empty());
    }

    #[test]
    fn test_partial_keymap_override() {
        // When a user provides only command keys, insert/search should use defaults
        let json = r#"{
            "key_map": {
                "command": {
                    "q": "quit",
                    "x": "quit"
                }
            }
        }"#;

        let cfg: Config = serde_json::from_str(json).unwrap();
        // User's command map replaces defaults entirely (serde behavior)
        assert_eq!(cfg.key_map["command"].get("q").unwrap(), "quit");
        assert_eq!(cfg.key_map["command"].get("x").unwrap(), "quit");
        // insert and search should be absent since user only provided command
        assert!(!cfg.key_map.contains_key("insert"));
    }
}
