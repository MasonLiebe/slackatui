# slackatui

A Slack client for your terminal, built with Rust and [ratatui](https://ratatui.rs).

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- Browse channels, groups, DMs, and multi-party DMs
- Send and receive messages in real time (via Slack RTM)
- Threaded conversations
- Vim-style keybindings (command / insert / search modes)
- Emoji shortcode rendering (`:thumbsup:` -> 👍)
- Mention expansion (`<@U12345>` -> `@username`)
- Channel search with `/` then `n`/`N` to cycle matches
- Slash commands (`/status`, `/away`, etc.)
- Token storage via macOS Keychain or encrypted JSON file

## Quickstart

### 1. Install Rust

If you don't have Rust installed:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Clone and build

```sh
git clone https://github.com/MasonLiebe/slackatui.git
cd slackatui
cargo build --release
```

The binary will be at `target/release/slackatui`. You can add it to your PATH:

```sh
# Option 1: copy to a system-wide location (requires sudo)
sudo cp target/release/slackatui /usr/local/bin/

# Option 2: copy to a user-local bin directory (no sudo needed)
mkdir -p ~/.local/bin
cp target/release/slackatui ~/.local/bin/
# Make sure ~/.local/bin is in your PATH — add this to ~/.zshrc or ~/.bashrc:
# export PATH="$HOME/.local/bin:$PATH"
```

### 3. Create a Slack App

You need a Slack App to authenticate. Go to [api.slack.com/apps](https://api.slack.com/apps) and click **Create New App** > **From scratch**.

1. **Name your app** (e.g. "slackatui") and select your workspace
2. Go to **OAuth & Permissions** in the sidebar
3. Under **Redirect URLs**, add: `https://localhost:8888`
4. Under **User Token Scopes**, add these scopes:
   - `channels:read`, `channels:history`, `channels:write`
   - `groups:read`, `groups:history`, `groups:write`
   - `im:read`, `im:history`, `im:write`
   - `mpim:read`, `mpim:history`, `mpim:write`
   - `chat:write`
   - `users:read`, `users:write`
5. Note your **Client ID** from the **Basic Information** page

### 4. Configure slackatui

Run `slackatui` once to generate a default config file, then edit it:

```sh
# Generate default config
slackatui

# Edit the config (macOS)
open ~/.config/slackatui/config

# Or with your editor
$EDITOR ~/.config/slackatui/config
```

Set your Client ID and redirect URI in the config JSON:

```json
{
  "auth": {
    "client_id": "YOUR_CLIENT_ID_HERE",
    "redirect_uri": "https://localhost:8888",
    "token_store": "keychain",
    "token_preference": "user"
  },
  "emoji": true,
  "sidebar_width": 1,
  "threads_width": 1
}
```

| Field | Description |
|---|---|
| `auth.client_id` | Your Slack App's Client ID (from step 3) |
| `auth.redirect_uri` | Must match what you set in the Slack App (`https://localhost:8888`) |
| `auth.token_store` | `"keychain"` (macOS Keychain) or `"file"` (JSON at `~/.config/slackatui/tokens.json`) |
| `auth.token_preference` | `"user"` (recommended) or `"bot"` |
| `auth.team_id` | Optional. Set to target a specific workspace if you have multiple |
| `emoji` | `true` to render emoji shortcodes as Unicode |
| `sidebar_width` | Channel list width (1-11, as a fraction of 12 columns) |
| `threads_width` | Thread panel width (1-11) |

### 5. Authenticate

```sh
slackatui auth
```

This will:
1. Generate a self-signed TLS certificate for localhost
2. Start a local HTTPS server on port 8888
3. Open your browser to Slack's OAuth page
4. After you approve, Slack redirects back to `https://localhost:8888`
5. Your browser may show a certificate warning — click **Advanced** > **Proceed** (this is expected for the self-signed cert)
6. Your token is saved to Keychain (or file, based on config)

You should see:
```
Authentication successful!
Team: Your Workspace Name
Tokens stored in: Keychain
```

### 6. Run

```sh
slackatui
```

## Keybindings

### Command mode (default)

| Key | Action |
|---|---|
| `i` | Enter insert mode (type messages) |
| `/` | Enter search mode (search channels) |
| `q` | Quit |
| `j` / `k` | Navigate channels down / up |
| `g` / `G` | Jump to first / last channel |
| `J` / `K` | Scroll thread down / up |
| `Ctrl-f` / `Ctrl-b` | Scroll chat down / up |
| `Ctrl-d` / `Ctrl-u` | Scroll chat down / up |
| `PgDn` / `PgUp` | Scroll chat down / up |
| `n` / `N` | Next / previous search match |
| `'` | Toggle thread panel |
| `F1` | Show help |

### Insert mode

| Key | Action |
|---|---|
| `Enter` | Send message |
| `Escape` | Return to command mode |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor |

### Search mode

| Key | Action |
|---|---|
| Type | Filter channels by name |
| `Enter` | Jump to match and exit search |
| `Escape` | Cancel search |

## Troubleshooting

**"No stored tokens found"** — Run `slackatui auth` first.

**"auth.client_id is not set"** — Edit `~/.config/slackatui/config` and add your Slack App's Client ID.

**OAuth flow hangs** — Make sure port 8888 is free and your redirect URI matches exactly (`https://localhost:8888`).

**Browser shows certificate warning** — This is expected. The auth flow uses a self-signed TLS certificate for the local callback server. Click "Advanced" and "Proceed to localhost" to complete the flow.

**"invalid_auth" or "not_authed"** — Your token may have expired. Re-run `slackatui auth`.

**No channels appear** — Make sure your Slack App has the required user token scopes (see step 3 above).

## Architecture

```
src/
├── main.rs          # Entry point, arg parsing, token loading
├── config.rs        # JSON config loading and validation
├── types.rs         # Core data types (ChannelItem, Message, Mode)
├── parse.rs         # Message parsing (mentions, emoji, HTML)
├── service.rs       # High-level Slack service (API -> display types)
├── auth/
│   ├── oauth.rs     # OAuth v2 flow with local callback server
│   └── store.rs     # Token storage (Keychain / file)
├── slack/
│   ├── client.rs    # Slack REST API client (reqwest)
│   └── rtm.rs       # RTM WebSocket with auto-reconnect
└── tui/
    ├── mod.rs       # App state, event loop, key dispatch
    └── layout.rs    # 3-pane ratatui layout rendering
```

## License

MIT
