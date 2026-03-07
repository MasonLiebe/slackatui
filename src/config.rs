use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
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
