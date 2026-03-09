# slackatui

A Slack client for your terminal, built with Rust and [ratatui](https://ratatui.rs).

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

<video src="slackatui-demo.mp4" controls autoplay muted loop width="100%"></video>

## Features

- Browse channels, groups, DMs, and multi-party DMs
- Send and receive messages with real-time polling
- Threaded conversations with reply count indicators
- Vim-style keybindings (command / insert / search modes)
- Focus-based pane navigation (channels -> chat -> thread)
- Message selection and highlighting
- Emoji shortcode rendering (`:thumbsup:` -> 👍)
- Mention expansion (`<@U12345>` -> `@username`)
- Clickable hyperlinks (OSC 8 terminal links)
- Inline image rendering (Sixel, Kitty, iTerm2, halfblocks)
- File uploads with drag-and-drop staging
- Channel search with `/` then `n`/`N` to cycle matches
- Slash commands (`/status`, `/away`, etc.)
- Desktop notifications with sound (macOS)
- Unread indicators across all channels
- Online presence display and toggle (`p` to go active/away)
- Emoji reactions picker (`e` to react)
- Interactive configuration wizard (`slackatui config`)
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
   - `reactions:read`, `reactions:write`
   - `files:read`, `files:write`
   - `users.profile:read`, `users.profile:write`
5. Note your **Client ID** and **Client Secret** from the **Basic Information** page (under "App Credentials")

### 4. Configure slackatui

Run the interactive configuration wizard:

```sh
slackatui config
```

This walks you through all settings including Slack credentials, notifications, emoji, layout, and theme. Or edit the config file directly:

```json
{
  "auth": {
    "client_id": "YOUR_CLIENT_ID_HERE",
    "client_secret": "YOUR_CLIENT_SECRET_HERE",
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
| `auth.client_secret` | Your Slack App's Client Secret (from step 3) |
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
1. Start a local HTTPS server on port 8888 (with a self-signed cert)
2. Open your browser to `https://localhost:8888`
3. Your browser will show a certificate warning — click **Advanced** > **Proceed to localhost** (this is normal for the self-signed cert)
4. The page auto-redirects to Slack's OAuth authorization page
5. After you approve, Slack redirects back to `https://localhost:8888` (cert already accepted)
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

The UI has three panes: **Channels** (sidebar), **Chat** (messages), and **Thread** (replies). You navigate between panes with `l` (right) and `h` (left). The active pane is highlighted with a green border.

### Command mode (default)

**Pane navigation:**

| Key | Action |
|---|---|
| `l` or `Enter` | Move focus right (Channels -> Chat -> open thread) |
| `h` | Move focus left (Thread -> Chat -> Channels) |
| `'` | Open thread for selected message |

**Within the focused pane:**

| Key | Action |
|---|---|
| `j` / `k` | Navigate down / up (channels, messages, or thread depending on focus) |
| `g` / `G` | Jump to first / last item |
| `Ctrl-f` / `Ctrl-b` | Page scroll down / up |
| `Ctrl-d` / `Ctrl-u` | Page scroll down / up |
| `PgDn` / `PgUp` | Page scroll down / up |

**Other:**

| Key | Action |
|---|---|
| `i` | Enter insert mode (type messages) |
| `/` | Enter search mode (search channels) |
| `n` / `N` | Next / previous search match |
| `r` | Reply to selected message in thread |
| `e` | Open emoji reaction picker |
| `o` | Open/view file attachment |
| `d` | Download file to directory (with tab completion) |
| `u` | Upload a file |
| `p` | Toggle presence (active/away) |
| `q` | Quit |
| `F1` | Show help |

### Insert mode

| Key | Action |
|---|---|
| `Enter` | Send message |
| `Shift+Enter` | Insert newline |
| `Escape` | Return to command mode |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor |
| `Tab` / `Shift+Tab` | Indent / dedent (bullet lists) |
| `Ctrl+b` | Toggle bold |
| `Ctrl+i` | Toggle italic |

### Search mode

| Key | Action |
|---|---|
| Type | Filter channels by name |
| `Enter` | Jump to match and exit search |
| `Escape` | Cancel search |

### Workflow example

1. Start in **Channels** pane — use `j`/`k` to pick a channel
2. Press `l` or `Enter` to move into **Chat** — messages are highlighted as you navigate with `j`/`k`
3. On a message with replies, press `l`, `Enter`, or `'` to open the **Thread** pane
4. Press `h` to go back (Thread -> Chat -> Channels)
5. Press `i` to type a message, `Enter` to send, `Escape` to return to command mode

## Troubleshooting

**"No stored tokens found"** — Run `slackatui auth` first.

**"auth.client_id is not set"** — Edit your config file and add your Slack App's Client ID. On macOS the path is `~/Library/Application Support/slackatui/config`. On Linux it's `~/.config/slackatui/config`.

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
│   └── rtm.rs       # RTM WebSocket for real-time events
└── tui/
    ├── mod.rs       # App state, event loop, key dispatch
    └── layout.rs    # 3-pane ratatui layout rendering
```

## License

MIT
