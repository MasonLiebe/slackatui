mod layout;

use crossterm::{
    event::{
        self, Event, KeyCode, KeyModifiers, KeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    },
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use std::collections::HashMap;
use std::io::{self, stdout};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::service::SlackService;
use crate::types::{ChannelItem, Focus, Message, Mode};

/// Application state shared across all TUI components.
pub struct App {
    pub config: Config,
    pub mode: Mode,
    pub focus: Focus,
    pub running: bool,

    // Channel list state
    pub channels: Vec<ChannelItem>,
    pub selected_channel: usize,
    pub channel_scroll: usize,

    // Chat messages for the selected channel
    pub messages: Vec<Message>,
    pub chat_scroll: usize,
    pub selected_message: Option<usize>,

    // Thread messages for the selected thread
    pub thread_messages: Vec<Message>,
    pub thread_scroll: usize,
    pub thread_visible: bool,

    // Input buffer
    pub input: String,
    pub cursor_pos: usize,
    pub reply_thread_ts: Option<String>,

    // Search state
    pub search_input: String,
    pub last_search_match: Option<usize>,

    // Current user info (for optimistic updates)
    pub current_user_name: String,
    pub current_user_id: String,

    // Reaction picker state
    pub react_query: String,
    pub react_results: Vec<(String, String)>, // (shortcode, emoji_char)
    pub react_selected: usize,

    // Image cache: file_id -> rendered pixel rows (each row = Vec of (fg, bg) color pairs)
    pub image_cache: HashMap<String, Vec<Vec<(Color, Color)>>>,
    pub image_pending: std::collections::HashSet<String>,

    // Status / mode indicator
    pub status: String,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            mode: Mode::Command,
            focus: Focus::Channels,
            running: true,
            channels: Vec::new(),
            selected_channel: 0,
            channel_scroll: 0,
            messages: Vec::new(),
            chat_scroll: 0,
            selected_message: None,
            thread_messages: Vec::new(),
            thread_scroll: 0,
            thread_visible: false,
            input: String::new(),
            cursor_pos: 0,
            reply_thread_ts: None,
            search_input: String::new(),
            last_search_match: None,
            current_user_name: String::new(),
            current_user_id: String::new(),
            react_query: String::new(),
            react_results: Vec::new(),
            react_selected: 0,
            image_cache: HashMap::new(),
            image_pending: std::collections::HashSet::new(),
            status: String::new(),
        }
    }

    /// Returns the currently selected channel, if any.
    pub fn current_channel(&self) -> Option<&ChannelItem> {
        self.channels.get(self.selected_channel)
    }

    /// Returns the mode-specific key map from config.
    pub fn current_keymap(&self) -> Option<&std::collections::HashMap<String, String>> {
        let mode_key = match self.mode {
            Mode::Command => "command",
            Mode::Insert => "insert",
            Mode::Search => "search",
            Mode::React => return None, // React mode handles keys inline
        };
        self.config.key_map.get(mode_key)
    }

    /// Move channel selection up.
    pub fn channel_up(&mut self) {
        if self.selected_channel > 0 {
            self.selected_channel -= 1;
        }
    }

    /// Move channel selection down.
    pub fn channel_down(&mut self) {
        if !self.channels.is_empty() && self.selected_channel < self.channels.len() - 1 {
            self.selected_channel += 1;
        }
    }

    /// Move channel selection to top.
    pub fn channel_top(&mut self) {
        self.selected_channel = 0;
    }

    /// Move channel selection to bottom.
    pub fn channel_bottom(&mut self) {
        if !self.channels.is_empty() {
            self.selected_channel = self.channels.len() - 1;
        }
    }

    /// Move message selection up.
    pub fn message_up(&mut self) {
        if let Some(idx) = self.selected_message {
            if idx > 0 {
                self.selected_message = Some(idx - 1);
            }
        }
    }

    /// Move message selection down.
    pub fn message_down(&mut self) {
        if let Some(idx) = self.selected_message {
            if idx + 1 < self.messages.len() {
                self.selected_message = Some(idx + 1);
            }
        }
    }

    /// Move message selection to top.
    pub fn message_top(&mut self) {
        if !self.messages.is_empty() {
            self.selected_message = Some(0);
        }
    }

    /// Move message selection to bottom.
    pub fn message_bottom(&mut self) {
        if !self.messages.is_empty() {
            self.selected_message = Some(self.messages.len() - 1);
        }
    }

    /// Scroll chat up by a page.
    pub fn chat_up(&mut self) {
        if self.selected_message.is_some() {
            if let Some(idx) = self.selected_message {
                self.selected_message = Some(idx.saturating_sub(10));
            }
        } else {
            self.chat_scroll = self.chat_scroll.saturating_add(10);
            // Clamp: rough upper bound is total message count * ~3 lines each
            let max_scroll = self.messages.len().saturating_mul(3);
            self.chat_scroll = self.chat_scroll.min(max_scroll);
        }
    }

    /// Scroll chat down by a page.
    pub fn chat_down(&mut self) {
        if self.selected_message.is_some() {
            if let Some(idx) = self.selected_message {
                let max = self.messages.len().saturating_sub(1);
                self.selected_message = Some((idx + 10).min(max));
            }
        } else {
            self.chat_scroll = self.chat_scroll.saturating_sub(10);
        }
    }

    /// Scroll thread up.
    pub fn thread_up(&mut self) {
        self.thread_scroll = self.thread_scroll.saturating_add(5);
        let max_scroll = self.thread_messages.len().saturating_mul(3);
        self.thread_scroll = self.thread_scroll.min(max_scroll);
    }

    /// Scroll thread down.
    pub fn thread_down(&mut self) {
        self.thread_scroll = self.thread_scroll.saturating_sub(5);
    }

    /// Insert a character at the cursor position.
    pub fn input_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Delete the character before the cursor.
    pub fn input_backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
            self.input.remove(self.cursor_pos);
        }
    }

    /// Delete the character at the cursor.
    pub fn input_delete(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.input.remove(self.cursor_pos);
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
        }
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            let next = self.input[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos += next;
        }
    }

    /// Search channels forward from current position for matching name.
    pub fn channel_search_next(&mut self) -> Option<usize> {
        if self.search_input.is_empty() || self.channels.is_empty() {
            return None;
        }
        let query = self.search_input.to_lowercase();
        let start = self.last_search_match.map(|i| i + 1).unwrap_or(0);

        // Search forward from start, wrapping around
        for offset in 0..self.channels.len() {
            let idx = (start + offset) % self.channels.len();
            if self.channels[idx].name.to_lowercase().contains(&query) {
                self.last_search_match = Some(idx);
                self.selected_channel = idx;
                return Some(idx);
            }
        }
        None
    }

    /// Search channels backward from current position.
    pub fn channel_search_prev(&mut self) -> Option<usize> {
        if self.search_input.is_empty() || self.channels.is_empty() {
            return None;
        }
        let query = self.search_input.to_lowercase();
        let start = self
            .last_search_match
            .unwrap_or(0)
            .checked_sub(1)
            .unwrap_or(self.channels.len() - 1);

        for offset in 0..self.channels.len() {
            let idx = (start + self.channels.len() - offset) % self.channels.len();
            if self.channels[idx].name.to_lowercase().contains(&query) {
                self.last_search_match = Some(idx);
                self.selected_channel = idx;
                return Some(idx);
            }
        }
        None
    }

    /// Take the current input buffer, resetting it.
    pub fn take_input(&mut self) -> String {
        self.cursor_pos = 0;
        std::mem::take(&mut self.input)
    }
}

/// Async entry point: initialize service, load channels, start RTM, run TUI.
pub async fn run_async(config: Config, mut svc: SlackService) -> io::Result<()> {
    // Initialize service (auth test + user cache)
    svc.init().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let mut app = App::new(config);
    app.current_user_id = svc.current_user_id.clone();
    app.current_user_name = svc
        .user_cache
        .get(&svc.current_user_id)
        .cloned()
        .unwrap_or_else(|| "me".to_string());
    app.status = "Loading channels...".to_string();

    // Load channels
    match svc.get_channels().await {
        Ok(channels) => {
            app.channels = channels;
            app.status.clear();
        }
        Err(e) => {
            app.status = format!("Error loading channels: {}", e);
        }
    }

    // Load initial messages for the first channel
    if let Some(ch) = app.current_channel() {
        let ch_id = ch.id.clone();
        match svc.get_messages(&ch_id, 50).await {
            Ok(msgs) => app.messages = msgs,
            Err(e) => app.status = format!("Error loading messages: {}", e),
        }
    }

    // Create a channel for async actions triggered by key events
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<AsyncAction>();

    // Queue image downloads for initial messages
    let initial_msgs = app.messages.clone();
    queue_image_downloads(&mut app, &initial_msgs, &action_tx);

    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    // Enable keyboard enhancement for Shift+Enter etc. (ignored if unsupported)
    let _ = stdout().execute(PushKeyboardEnhancementFlags(
        KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES,
    ));
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Splash screen
    show_splash(&mut terminal).await?;

    // Main loop
    let result = async_main_loop(
        &mut terminal,
        &mut app,
        &mut svc,
        &action_tx,
        &mut action_rx,
    )
    .await;

    // Restore terminal
    let _ = stdout().execute(PopKeyboardEnhancementFlags);
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

/// Show an animated splash screen. Typewriter effect, then wait for any key.
async fn show_splash(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    // Total characters to animate (logo + welcome message)
    // We'll reveal ~4 chars per frame at 30ms intervals for a snappy typewriter
    let total_frames = 80; // enough to cover logo + message + hint
    let chars_per_frame = 5;

    for frame_idx in 0..=total_frames {
        let char_count = frame_idx * chars_per_frame;
        terminal.draw(|f| {
            layout::render_splash(f, char_count);
        })?;

        // Check if user pressed a key to skip
        if event::poll(Duration::from_millis(18))? {
            if let Event::Key(_) = event::read()? {
                // Draw the fully revealed splash once, then break
                terminal.draw(|f| {
                    layout::render_splash(f, 9999);
                })?;
                break;
            }
        }
    }

    // Wait for any key press to dismiss (with a timeout so it auto-dismisses)
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }
    }

    Ok(())
}

/// Actions that need async processing (sent from key handler to main loop).
enum AsyncAction {
    SendMessage { text: String, thread_ts: Option<String> },
    SelectChannel { index: usize },
    OpenThread { channel_id: String, thread_ts: String },
    ToggleReaction { channel_id: String, timestamp: String, emoji_name: String, msg_idx: usize },
    DownloadImage { file_id: String, url: String },
    ImageReady { file_id: String, rows: Vec<Vec<(Color, Color)>> },
    ImageFailed { file_id: String },
}

const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// The async main event loop: render, poll for keyboard events and new messages.
async fn async_main_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    svc: &mut SlackService,
    action_tx: &mpsc::UnboundedSender<AsyncAction>,
    action_rx: &mut mpsc::UnboundedReceiver<AsyncAction>,
) -> io::Result<()> {
    let mut poll_timer = tokio::time::interval(POLL_INTERVAL);
    // Skip the first immediate tick
    poll_timer.tick().await;

    while app.running {
        terminal.draw(|frame| {
            layout::render(frame, app);
        })?;

        tokio::select! {
            // Check for keyboard events (non-blocking poll)
            _ = tokio::task::yield_now() => {
                if event::poll(Duration::from_millis(50))? {
                    if let Event::Key(key) = event::read()? {
                        handle_key_async(app, key.code, key.modifiers, action_tx);
                    }
                }
            }

            // Poll for new messages periodically
            _ = poll_timer.tick() => {
                poll_new_messages(app, svc, action_tx).await;
            }

            // Async actions from key handlers
            Some(action) = action_rx.recv() => {
                handle_async_action(app, svc, action, action_tx).await;
            }
        }
    }

    Ok(())
}

/// Poll the active channel for new messages since the last known message.
async fn poll_new_messages(
    app: &mut App,
    svc: &mut SlackService,
    action_tx: &mpsc::UnboundedSender<AsyncAction>,
) {
    let (ch_id, oldest_ts) = match app.current_channel() {
        Some(ch) => {
            // Use the last real (non-optimistic) message timestamp
            let oldest = app
                .messages
                .iter()
                .rev()
                .find(|m| !m.timestamp.starts_with("optimistic_"))
                .map(|m| m.timestamp.clone())
                .unwrap_or_default();
            (ch.id.clone(), oldest)
        }
        None => return,
    };

    if oldest_ts.is_empty() {
        return;
    }

    match svc.get_new_messages(&ch_id, &oldest_ts).await {
        Ok(new_msgs) if !new_msgs.is_empty() => {
            // Remove optimistic messages — real ones are arriving
            app.messages.retain(|m| !m.timestamp.starts_with("optimistic_"));
            queue_image_downloads(app, &new_msgs, action_tx);
            app.messages.extend(new_msgs);
            app.chat_scroll = 0; // Scroll to bottom on new messages
        }
        Err(_) => {
            // Silently ignore poll errors to avoid spamming the status bar
        }
        _ => {}
    }
}

/// Handle async actions triggered by key events.
/// Queue image downloads for any image files in the given messages that aren't already cached/pending.
fn queue_image_downloads(
    app: &mut App,
    messages: &[Message],
    action_tx: &mpsc::UnboundedSender<AsyncAction>,
) {
    for msg in messages {
        for img in &msg.image_files {
            if !app.image_cache.contains_key(&img.file_id) && !app.image_pending.contains(&img.file_id) {
                app.image_pending.insert(img.file_id.clone());
                let _ = action_tx.send(AsyncAction::DownloadImage {
                    file_id: img.file_id.clone(),
                    url: img.url.clone(),
                });
            }
        }
    }
}

async fn handle_async_action(
    app: &mut App,
    svc: &mut SlackService,
    action: AsyncAction,
    action_tx: &mpsc::UnboundedSender<AsyncAction>,
) {
    match action {
        AsyncAction::SendMessage { text, thread_ts } => {
            if let Some(ch) = app.current_channel() {
                let ch_id = ch.id.clone();
                if let Err(e) = svc.send(&ch_id, &text, thread_ts.as_deref()).await {
                    app.status = format!("Send error: {}", e);
                } else if let Some(ts) = &thread_ts {
                    // Refresh thread to replace optimistic message with real data
                    if let Ok(msgs) = svc.get_thread_messages(&ch_id, ts).await {
                        app.thread_messages = msgs;
                        app.thread_scroll = 0;
                    }
                }
            }
        }
        AsyncAction::SelectChannel { index } => {
            if index < app.channels.len() {
                app.selected_channel = index;
                app.messages.clear();
                app.chat_scroll = 0;
                app.selected_message = None;
                app.thread_messages.clear();
                app.thread_visible = false;

                let ch_id = app.channels[index].id.clone();
                app.channels[index].notification = false;

                match svc.get_messages(&ch_id, 50).await {
                    Ok(msgs) => {
                        app.messages = msgs;
                        let msgs_ref = app.messages.clone();
                        queue_image_downloads(app, &msgs_ref, action_tx);
                    }
                    Err(e) => app.status = format!("Error: {}", e),
                }
            }
        }
        AsyncAction::OpenThread { channel_id, thread_ts } => {
            match svc.get_thread_messages(&channel_id, &thread_ts).await {
                Ok(msgs) => {
                    app.thread_messages = msgs;
                    app.thread_visible = true;
                    app.thread_scroll = 0;
                }
                Err(e) => app.status = format!("Thread error: {}", e),
            }
        }
        AsyncAction::ToggleReaction { channel_id, timestamp, emoji_name, msg_idx } => {
            // Check if we already reacted — toggle
            let already_reacted = app.messages
                .get(msg_idx)
                .map(|m| m.reactions.iter().any(|r| r.name == emoji_name && r.reacted))
                .unwrap_or(false);

            let result = if already_reacted {
                svc.client.remove_reaction(&channel_id, &timestamp, &emoji_name).await
            } else {
                svc.client.add_reaction(&channel_id, &timestamp, &emoji_name).await
            };

            match result {
                Ok(()) => {
                    // Optimistic update
                    if let Some(msg) = app.messages.get_mut(msg_idx) {
                        if already_reacted {
                            // Decrement or remove
                            if let Some(r) = msg.reactions.iter_mut().find(|r| r.name == emoji_name) {
                                r.count = r.count.saturating_sub(1);
                                r.reacted = false;
                                if r.count == 0 {
                                    msg.reactions.retain(|r| r.name != emoji_name);
                                }
                            }
                        } else {
                            // Increment or add
                            if let Some(r) = msg.reactions.iter_mut().find(|r| r.name == emoji_name) {
                                r.count += 1;
                                r.reacted = true;
                            } else {
                                let emoji = crate::parse::resolve_emoji(&emoji_name);
                                msg.reactions.push(crate::types::Reaction {
                                    name: emoji_name,
                                    emoji,
                                    count: 1,
                                    reacted: true,
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    app.status = format!("Reaction error: {}", e);
                }
            }
        }
        AsyncAction::DownloadImage { file_id, url } => {
            // Spawn as background task to avoid blocking the UI
            let token = svc.client.token().to_string();
            let fid = file_id.clone();
            let tx = action_tx.clone();
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let result = client
                    .get(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .and_then(|r| Ok(r));
                match result {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(bytes) = resp.bytes().await {
                            if let Ok(img) = image::load_from_memory(&bytes) {
                                let max_width: u32 = 60;
                                let max_height: u32 = 30;
                                let img = img.thumbnail(max_width, max_height * 2);
                                let rgba = img.to_rgba8();
                                let (w, h) = rgba.dimensions();

                                let mut rows: Vec<Vec<(Color, Color)>> = Vec::new();
                                let mut y = 0u32;
                                while y < h {
                                    let mut row = Vec::new();
                                    for x in 0..w {
                                        let top = rgba.get_pixel(x, y);
                                        let bottom = if y + 1 < h {
                                            rgba.get_pixel(x, y + 1)
                                        } else {
                                            top
                                        };
                                        let fg = Color::Rgb(top[0], top[1], top[2]);
                                        let bg = Color::Rgb(bottom[0], bottom[1], bottom[2]);
                                        row.push((fg, bg));
                                    }
                                    rows.push(row);
                                    y += 2;
                                }

                                let _ = tx.send(AsyncAction::ImageReady {
                                    file_id: fid,
                                    rows,
                                });
                                return;
                            }
                        }
                        let _ = tx.send(AsyncAction::ImageFailed { file_id: fid });
                    }
                    _ => {
                        let _ = tx.send(AsyncAction::ImageFailed { file_id: fid });
                    }
                }
            });
        }
        AsyncAction::ImageReady { file_id, rows } => {
            app.image_cache.insert(file_id.clone(), rows);
            app.image_pending.remove(&file_id);
        }
        AsyncAction::ImageFailed { file_id } => {
            app.image_pending.remove(&file_id);
        }
    }
}

/// Key handler that can trigger async actions via the action channel.
fn handle_key_async(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
    action_tx: &mpsc::UnboundedSender<AsyncAction>,
) {
    let key_str = key_to_string(code, modifiers);

    match app.mode {
        Mode::Insert => {
            if let Some(keymap) = app.current_keymap() {
                if let Some(action) = keymap.get(&key_str) {
                    dispatch_action(app, action.clone(), action_tx);
                    return;
                }
            }
            if let KeyCode::Char(c) = code {
                let ch = if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                app.input_char(ch);
            }
        }
        Mode::Search => {
            if let Some(keymap) = app.current_keymap() {
                if let Some(action) = keymap.get(&key_str) {
                    dispatch_action(app, action.clone(), action_tx);
                    return;
                }
            }
            if let KeyCode::Char(c) = code {
                let ch = if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                app.search_input.push(ch);
            }
        }
        Mode::React => {
            match code {
                KeyCode::Esc => {
                    app.mode = Mode::Command;
                    app.react_query.clear();
                    app.react_results.clear();
                    app.status.clear();
                }
                KeyCode::Enter => {
                    // Select the highlighted emoji and send reaction
                    if let Some((shortcode, _)) = app.react_results.get(app.react_selected).cloned() {
                        if let Some(msg_idx) = app.selected_message {
                            if let Some(msg) = app.messages.get(msg_idx) {
                                let ch_id = app.current_channel()
                                    .map(|c| c.id.clone())
                                    .unwrap_or_default();
                                let ts = msg.timestamp.clone();
                                let _ = action_tx.send(AsyncAction::ToggleReaction {
                                    channel_id: ch_id,
                                    timestamp: ts,
                                    emoji_name: shortcode,
                                    msg_idx,
                                });
                            }
                        }
                    }
                    app.mode = Mode::Command;
                    app.react_query.clear();
                    app.react_results.clear();
                    app.status.clear();
                }
                KeyCode::Backspace => {
                    app.react_query.pop();
                    app.react_selected = 0;
                    update_react_results(app);
                }
                KeyCode::Up => {
                    app.react_selected = app.react_selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if !app.react_results.is_empty() {
                        app.react_selected = (app.react_selected + 1).min(app.react_results.len() - 1);
                    }
                }
                KeyCode::Char(c) => {
                    let ch = if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    };
                    app.react_query.push(ch);
                    app.react_selected = 0;
                    update_react_results(app);
                }
                _ => {}
            }
        }
        Mode::Command => {
            if let Some(keymap) = app.current_keymap() {
                if let Some(action) = keymap.get(&key_str) {
                    dispatch_action(app, action.clone(), action_tx);
                }
            }
        }
    }
}

/// Dispatch a named action from the keymap.
fn dispatch_action(
    app: &mut App,
    action: String,
    action_tx: &mpsc::UnboundedSender<AsyncAction>,
) {
    match action.as_str() {
        // Mode switching
        "mode-insert" => {
            app.mode = Mode::Insert;
            app.reply_thread_ts = None;
            app.status = "-- INSERT --".to_string();
        }
        "reply" => {
            // Enter insert mode replying to the selected message's thread
            if let Some(idx) = app.selected_message {
                if let Some(msg) = app.messages.get(idx) {
                    let thread_ts = if !msg.thread.is_empty() {
                        msg.thread.clone()
                    } else {
                        msg.timestamp.clone()
                    };
                    let reply_to_name = msg.name.clone();
                    app.reply_thread_ts = Some(thread_ts);
                    app.mode = Mode::Insert;
                    app.status = format!("replying to {}", reply_to_name);
                }
            } else {
                app.mode = Mode::Insert;
                app.reply_thread_ts = None;
                app.status = "-- INSERT --".to_string();
            }
        }
        "mode-command" => {
            app.mode = Mode::Command;
            app.reply_thread_ts = None;
            app.status.clear();
        }
        "mode-search" => {
            app.mode = Mode::Search;
            app.search_input.clear();
            app.status = "/".to_string();
        }
        "mode-react" => {
            if app.selected_message.is_some() {
                app.mode = Mode::React;
                app.react_query.clear();
                app.react_selected = 0;
                // Show popular emojis by default
                app.react_results = POPULAR_EMOJIS.iter()
                    .map(|&(name, emoji)| (name.to_string(), emoji.to_string()))
                    .collect();
                app.status = "React: type to search emoji".to_string();
            }
        }

        // Focus-aware up/down navigation
        "channel-up" => match app.focus {
            Focus::Channels => {
                let prev = app.selected_channel;
                app.channel_up();
                if app.selected_channel != prev {
                    let _ = action_tx.send(AsyncAction::SelectChannel {
                        index: app.selected_channel,
                    });
                }
            }
            Focus::Chat => {
                app.message_up();
                hide_thread_if_no_replies(app);
            }
            Focus::Thread => app.thread_up(),
        },
        "channel-down" => match app.focus {
            Focus::Channels => {
                let prev = app.selected_channel;
                app.channel_down();
                if app.selected_channel != prev {
                    let _ = action_tx.send(AsyncAction::SelectChannel {
                        index: app.selected_channel,
                    });
                }
            }
            Focus::Chat => {
                app.message_down();
                hide_thread_if_no_replies(app);
            }
            Focus::Thread => app.thread_down(),
        },
        "channel-top" => match app.focus {
            Focus::Channels => {
                let prev = app.selected_channel;
                app.channel_top();
                if app.selected_channel != prev {
                    let _ = action_tx.send(AsyncAction::SelectChannel {
                        index: app.selected_channel,
                    });
                }
            }
            Focus::Chat => {
                app.message_top();
                hide_thread_if_no_replies(app);
            }
            Focus::Thread => {}
        },
        "channel-bottom" => match app.focus {
            Focus::Channels => {
                let prev = app.selected_channel;
                app.channel_bottom();
                if app.selected_channel != prev {
                    let _ = action_tx.send(AsyncAction::SelectChannel {
                        index: app.selected_channel,
                    });
                }
            }
            Focus::Chat => {
                app.message_bottom();
                hide_thread_if_no_replies(app);
            }
            Focus::Thread => {}
        },

        // Focus navigation
        "focus-right" | "select" => match app.focus {
            Focus::Channels => {
                app.focus = Focus::Chat;
                if app.selected_message.is_none() && !app.messages.is_empty() {
                    app.selected_message = Some(app.messages.len() - 1);
                }
            }
            Focus::Chat => {
                try_open_thread(app, action_tx);
            }
            Focus::Thread => {}
        },
        "focus-left" => match app.focus {
            Focus::Thread => {
                app.focus = Focus::Chat;
            }
            Focus::Chat => {
                app.focus = Focus::Channels;
                app.selected_message = None;
            }
            Focus::Channels => {}
        },
        "open-thread" => {
            if app.focus == Focus::Channels {
                // Enter chat first, then try thread
                app.focus = Focus::Chat;
                if app.selected_message.is_none() && !app.messages.is_empty() {
                    app.selected_message = Some(app.messages.len() - 1);
                }
            }
            try_open_thread(app, action_tx);
        }

        // Chat scrolling
        "chat-up" => app.chat_up(),
        "chat-down" => app.chat_down(),

        // Thread scrolling (always operates on thread pane)
        "thread-up" => app.thread_up(),
        "thread-down" => app.thread_down(),

        // Input editing
        "cursor-left" => app.cursor_left(),
        "cursor-right" => app.cursor_right(),
        "backspace" => {
            if app.mode == Mode::Search {
                app.search_input.pop();
            } else {
                app.input_backspace();
            }
        }
        "delete" => {
            if app.mode == Mode::Search {
                // no-op
            } else {
                app.input_delete();
            }
        }
        "space" => {
            if app.mode == Mode::Search {
                app.search_input.push(' ');
            } else {
                app.input_char(' ');
            }
        }
        "newline" => {
            let (_, line_before) = current_line_info(&app.input, app.cursor_pos);
            let prefix = bullet_prefix(line_before).to_string();
            app.input_char('\n');
            if !prefix.is_empty() {
                // Auto-continue bullet on new line
                for c in prefix.chars() {
                    app.input_char(c);
                }
            }
        }
        "indent" => {
            let (line_start, line_before) = current_line_info(&app.input, app.cursor_pos);
            let prefix = bullet_prefix(line_before);
            if !prefix.is_empty() {
                // Add 2 spaces at line start to indent the bullet
                app.input.insert_str(line_start, "  ");
                app.cursor_pos += 2;
            }
        }
        "dedent" => {
            let (line_start, line_before) = current_line_info(&app.input, app.cursor_pos);
            let prefix = bullet_prefix(line_before);
            if !prefix.is_empty() && line_before.starts_with("  ") {
                // Remove 2 spaces from line start
                app.input.drain(line_start..line_start + 2);
                app.cursor_pos -= 2;
            }
        }
        "toggle-bold" => {
            insert_wrap_markers(app, '*');
        }
        "toggle-italic" => {
            insert_wrap_markers(app, '_');
        }
        "toggle-underline" => {
            // Slack doesn't have underline, but we can approximate with italic
            insert_wrap_markers(app, '_');
        }

        // Send message
        "send" => {
            let raw = app.take_input();
            let thread_ts = app.reply_thread_ts.take();
            // Convert hyphen bullets to Unicode bullets
            let text = convert_bullets(&raw);
            if !text.is_empty() {
                // Optimistic update: show message immediately
                let optimistic = Message::new(
                    format!("optimistic_{}", app.messages.len()),
                    app.current_user_name.clone(),
                    text.clone(),
                    chrono::Local::now(),
                );
                if let Some(ref ts) = thread_ts {
                    // Thread reply: add to thread pane and bump parent reply count
                    app.thread_messages.push(optimistic);
                    app.thread_scroll = 0;
                    for msg in &mut app.messages {
                        if msg.timestamp == *ts || msg.thread == *ts {
                            msg.reply_count += 1;
                            break;
                        }
                    }
                } else {
                    app.messages.push(optimistic);
                }
                let _ = action_tx.send(AsyncAction::SendMessage { text, thread_ts: thread_ts.clone() });
            }
            app.mode = Mode::Command;
            if thread_ts.is_none() {
                // Regular message: scroll to bottom, deselect
                app.chat_scroll = 0;
                app.selected_message = None;
            }
            // Thread reply: keep selected_message so focus stays on parent
            app.status.clear();
        }

        // Search
        "clear-input" => {
            if !app.search_input.is_empty() {
                if let Some(idx) = app.channel_search_next() {
                    let _ = action_tx.send(AsyncAction::SelectChannel { index: idx });
                }
            }
            app.search_input.clear();
            app.last_search_match = None;
            app.mode = Mode::Command;
            app.status.clear();
        }
        "channel-search-next" => {
            if let Some(idx) = app.channel_search_next() {
                let _ = action_tx.send(AsyncAction::SelectChannel { index: idx });
            }
        }
        "channel-search-prev" => {
            if let Some(idx) = app.channel_search_prev() {
                let _ = action_tx.send(AsyncAction::SelectChannel { index: idx });
            }
        }

        // Quit
        "quit" => app.running = false,

        // Help
        "help" => {
            app.status =
                "j/k=nav l=enter h=back i=insert /=search '=thread q=quit".to_string();
        }

        _ => {}
    }
}

/// Get the current line's content (from the last newline before cursor to the next newline or end).
/// Returns (line_start_byte_offset, line_text_before_cursor).
fn current_line_info(input: &str, cursor_pos: usize) -> (usize, &str) {
    let before = &input[..cursor_pos];
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    (line_start, &before[line_start..])
}

/// Extract the bullet prefix from a line (e.g. "  - " -> "  - ", "- " -> "- ", "" otherwise).
fn bullet_prefix(line: &str) -> &str {
    // Match optional leading whitespace followed by "- "
    let trimmed = line.trim_start();
    if trimmed.starts_with("- ") {
        let indent_len = line.len() - trimmed.len();
        &line[..indent_len + 2] // indent + "- "
    } else {
        ""
    }
}

/// Convert `- ` at the start of lines to `• ` (Unicode bullet).
/// Preserves leading whitespace for indented bullets.
fn convert_bullets(text: &str) -> String {
    text.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("- ") {
                let indent = &line[..line.len() - trimmed.len()];
                format!("{}• {}", indent, &trimmed[2..])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Insert formatting markers (e.g. `*` for bold) around the cursor.
/// Places `marker` on both sides and positions cursor between them.
fn insert_wrap_markers(app: &mut App, marker: char) {
    app.input.insert(app.cursor_pos, marker);
    app.cursor_pos += marker.len_utf8();
    app.input.insert(app.cursor_pos, marker);
    // Cursor stays between the two markers
}

/// Hide the thread pane if the currently selected message has no replies.
fn hide_thread_if_no_replies(app: &mut App) {
    if !app.thread_visible {
        return;
    }
    let has_thread = app
        .selected_message
        .and_then(|idx| app.messages.get(idx))
        .map(|msg| msg.reply_count > 0 || !msg.thread.is_empty())
        .unwrap_or(false);
    if !has_thread {
        app.thread_visible = false;
        app.thread_messages.clear();
        if app.focus == Focus::Thread {
            app.focus = Focus::Chat;
        }
    }
}

/// Try to open the thread for the currently selected message.
/// Popular emojis shown as defaults in the reaction picker.
const POPULAR_EMOJIS: &[(&str, &str)] = &[
    ("thumbsup", "\u{1F44D}"),
    ("thumbsdown", "\u{1F44E}"),
    ("heart", "\u{2764}\u{FE0F}"),
    ("smile", "\u{1F604}"),
    ("joy", "\u{1F602}"),
    ("fire", "\u{1F525}"),
    ("tada", "\u{1F389}"),
    ("eyes", "\u{1F440}"),
    ("rocket", "\u{1F680}"),
    ("pray", "\u{1F64F}"),
    ("100", "\u{1F4AF}"),
    ("white_check_mark", "\u{2705}"),
    ("wave", "\u{1F44B}"),
    ("clap", "\u{1F44F}"),
    ("raised_hands", "\u{1F64C}"),
    ("thinking_face", "\u{1F914}"),
    ("muscle", "\u{1F4AA}"),
    ("sparkles", "\u{2728}"),
    ("star", "\u{2B50}"),
    ("warning", "\u{26A0}\u{FE0F}"),
    ("ok_hand", "\u{1F44C}"),
    ("brain", "\u{1F9E0}"),
    ("skull", "\u{1F480}"),
    ("sunglasses", "\u{1F60E}"),
    ("beer", "\u{1F37A}"),
    ("coffee", "\u{2615}"),
    ("heavy_check_mark", "\u{2714}\u{FE0F}"),
    ("x", "\u{274C}"),
    ("point_up", "\u{261D}\u{FE0F}"),
    ("+1", "\u{1F44D}"),
];

/// Search the emoji alias table + emojis crate for matches.
fn update_react_results(app: &mut App) {
    if app.react_query.is_empty() {
        app.react_results = POPULAR_EMOJIS.iter()
            .map(|&(name, emoji)| (name.to_string(), emoji.to_string()))
            .collect();
        return;
    }

    let query = app.react_query.to_lowercase();
    let mut results: Vec<(String, String)> = Vec::new();

    // Search popular emojis first
    for &(name, emoji) in POPULAR_EMOJIS {
        if name.contains(&query) {
            results.push((name.to_string(), emoji.to_string()));
        }
    }

    // Search the full alias table
    let aliases = &*crate::parse::SLACK_EMOJI_ALIASES;
    for (&name, &emoji) in aliases.iter() {
        if name.contains(&query) && !results.iter().any(|(n, _)| n == name) {
            results.push((name.to_string(), emoji.to_string()));
        }
    }

    // Also search the emojis crate
    for emoji in emojis::iter() {
        if let Some(shortcode) = emoji.shortcode() {
            if shortcode.contains(&query) && !results.iter().any(|(n, _)| n == shortcode) {
                results.push((shortcode.to_string(), emoji.as_str().to_string()));
            }
        }
    }

    // Limit results
    results.truncate(20);
    app.react_results = results;
}

fn try_open_thread(app: &mut App, action_tx: &mpsc::UnboundedSender<AsyncAction>) {
    let Some(idx) = app.selected_message else {
        return;
    };
    let Some(msg) = app.messages.get(idx) else {
        return;
    };

    // Determine the thread_ts: for thread parents thread == timestamp,
    // for replies thread points to parent. Either way, use it.
    let thread_ts = if !msg.thread.is_empty() {
        msg.thread.clone()
    } else if msg.reply_count > 0 {
        msg.timestamp.clone()
    } else {
        return;
    };

    let ch_id = app
        .current_channel()
        .map(|c| c.id.clone())
        .unwrap_or_default();

    let _ = action_tx.send(AsyncAction::OpenThread {
        channel_id: ch_id,
        thread_ts,
    });
    app.thread_visible = true;
    app.focus = Focus::Thread;
}

/// Convert a crossterm key event to the config key string format.
fn key_to_string(code: KeyCode, modifiers: KeyModifiers) -> String {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let sup = modifiers.contains(KeyModifiers::SUPER);
    let shift = modifiers.contains(KeyModifiers::SHIFT);

    match code {
        KeyCode::Char(c) => {
            if ctrl || sup {
                format!("C-{}", c)
            } else if shift && c.is_ascii_lowercase() {
                // Keyboard enhancement reports Shift+a as Char('a') + SHIFT
                c.to_ascii_uppercase().to_string()
            } else {
                c.to_string()
            }
        }
        KeyCode::Enter => {
            if shift {
                "<s-enter>".to_string()
            } else {
                "<enter>".to_string()
            }
        }
        KeyCode::Esc => "<escape>".to_string(),
        KeyCode::Backspace => "<backspace>".to_string(),
        KeyCode::Delete => "<delete>".to_string(),
        KeyCode::Left => "<left>".to_string(),
        KeyCode::Right => "<right>".to_string(),
        KeyCode::Up => "<up>".to_string(),
        KeyCode::Down => "<down>".to_string(),
        KeyCode::PageUp => "<previous>".to_string(),
        KeyCode::PageDown => "<next>".to_string(),
        KeyCode::Home => "<home>".to_string(),
        KeyCode::End => "<end>".to_string(),
        KeyCode::Tab => "<tab>".to_string(),
        KeyCode::BackTab => "<s-tab>".to_string(),
        KeyCode::F(n) => format!("<f{}>", n),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new(Config::default())
    }

    fn test_action_tx() -> mpsc::UnboundedSender<AsyncAction> {
        let (tx, _rx) = mpsc::unbounded_channel();
        tx
    }

    #[test]
    fn test_app_initial_state() {
        let app = test_app();
        assert_eq!(app.mode, Mode::Command);
        assert_eq!(app.focus, Focus::Channels);
        assert!(app.running);
        assert!(app.channels.is_empty());
        assert!(app.messages.is_empty());
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_channel_navigation() {
        let mut app = test_app();
        // Add some channels
        for i in 0..5 {
            app.channels.push(ChannelItem::new(
                format!("C{}", i),
                format!("channel-{}", i),
                crate::types::ChannelType::Channel,
            ));
        }

        assert_eq!(app.selected_channel, 0);

        app.channel_down();
        assert_eq!(app.selected_channel, 1);

        app.channel_down();
        app.channel_down();
        assert_eq!(app.selected_channel, 3);

        app.channel_up();
        assert_eq!(app.selected_channel, 2);

        app.channel_top();
        assert_eq!(app.selected_channel, 0);

        app.channel_bottom();
        assert_eq!(app.selected_channel, 4);

        // Can't go past bottom
        app.channel_down();
        assert_eq!(app.selected_channel, 4);

        // Can't go past top
        app.channel_top();
        app.channel_up();
        assert_eq!(app.selected_channel, 0);
    }

    #[test]
    fn test_channel_navigation_empty() {
        let mut app = test_app();
        app.channel_down();
        assert_eq!(app.selected_channel, 0);
        app.channel_up();
        assert_eq!(app.selected_channel, 0);
        app.channel_bottom();
        assert_eq!(app.selected_channel, 0);
    }

    #[test]
    fn test_input_operations() {
        let mut app = test_app();

        app.input_char('h');
        app.input_char('i');
        assert_eq!(app.input, "hi");
        assert_eq!(app.cursor_pos, 2);

        app.input_backspace();
        assert_eq!(app.input, "h");
        assert_eq!(app.cursor_pos, 1);

        app.input_char('e');
        app.input_char('y');
        assert_eq!(app.input, "hey");

        app.cursor_left();
        app.cursor_left();
        assert_eq!(app.cursor_pos, 1);

        app.input_char('!');
        assert_eq!(app.input, "h!ey");

        app.input_delete();
        assert_eq!(app.input, "h!y");
    }

    #[test]
    fn test_input_cursor_bounds() {
        let mut app = test_app();

        // Cursor left at 0 should stay at 0
        app.cursor_left();
        assert_eq!(app.cursor_pos, 0);

        // Cursor right at end should stay at end
        app.input_char('a');
        app.cursor_right();
        assert_eq!(app.cursor_pos, 1);

        // Backspace at 0 should be no-op
        app.cursor_pos = 0;
        app.input_backspace();
        assert_eq!(app.input, "a");

        // Delete at end should be no-op
        app.cursor_pos = 1;
        app.input_delete();
        assert_eq!(app.input, "a");
    }

    #[test]
    fn test_take_input() {
        let mut app = test_app();
        app.input = "hello".to_string();
        app.cursor_pos = 5;

        let taken = app.take_input();
        assert_eq!(taken, "hello");
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_chat_scroll() {
        let mut app = test_app();
        // Add enough messages so scroll has room
        for i in 0..20 {
            app.messages.push(crate::types::Message::new(
                format!("{}.0", i),
                "user".into(),
                "msg".into(),
                chrono::Local::now(),
            ));
        }
        app.chat_up();
        assert_eq!(app.chat_scroll, 10);
        app.chat_up();
        assert_eq!(app.chat_scroll, 20);
        app.chat_down();
        assert_eq!(app.chat_scroll, 10);
        app.chat_down();
        assert_eq!(app.chat_scroll, 0);
        // Can't go negative
        app.chat_down();
        assert_eq!(app.chat_scroll, 0);
    }

    #[test]
    fn test_chat_scroll_clamped() {
        let mut app = test_app();
        // With only 2 messages, max scroll = 2*3 = 6
        for i in 0..2 {
            app.messages.push(crate::types::Message::new(
                format!("{}.0", i),
                "user".into(),
                "msg".into(),
                chrono::Local::now(),
            ));
        }
        app.chat_up();
        assert_eq!(app.chat_scroll, 6); // clamped to 2*3
        app.chat_up();
        assert_eq!(app.chat_scroll, 6); // stays clamped
    }

    #[test]
    fn test_thread_scroll() {
        let mut app = test_app();
        // Add enough thread messages so scroll has room
        for i in 0..10 {
            app.thread_messages.push(crate::types::Message::new(
                format!("{}.0", i),
                "user".into(),
                "msg".into(),
                chrono::Local::now(),
            ));
        }
        app.thread_up();
        assert_eq!(app.thread_scroll, 5);
        app.thread_down();
        assert_eq!(app.thread_scroll, 0);
        app.thread_down();
        assert_eq!(app.thread_scroll, 0);
    }

    #[test]
    fn test_dispatch_mode_switching() {
        let mut app = test_app();
        let tx = test_action_tx();

        dispatch_action(&mut app, "mode-insert".to_string(), &tx);
        assert_eq!(app.mode, Mode::Insert);
        assert_eq!(app.status, "-- INSERT --");

        dispatch_action(&mut app, "mode-command".to_string(), &tx);
        assert_eq!(app.mode, Mode::Command);
        assert!(app.status.is_empty());

        dispatch_action(&mut app, "mode-search".to_string(), &tx);
        assert_eq!(app.mode, Mode::Search);
    }

    #[test]
    fn test_dispatch_quit() {
        let mut app = test_app();
        let tx = test_action_tx();
        assert!(app.running);
        dispatch_action(&mut app, "quit".to_string(), &tx);
        assert!(!app.running);
    }

    #[test]
    fn test_dispatch_send() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;
        app.input = "hello world".to_string();
        app.cursor_pos = 11;

        dispatch_action(&mut app, "send".to_string(), &tx);
        assert!(app.input.is_empty());
        assert_eq!(app.mode, Mode::Command);
    }

    #[test]
    fn test_key_to_string() {
        assert_eq!(key_to_string(KeyCode::Char('j'), KeyModifiers::NONE), "j");
        assert_eq!(key_to_string(KeyCode::Char('J'), KeyModifiers::SHIFT), "J");
        assert_eq!(key_to_string(KeyCode::Char('b'), KeyModifiers::CONTROL), "C-b");
        assert_eq!(key_to_string(KeyCode::Enter, KeyModifiers::NONE), "<enter>");
        assert_eq!(key_to_string(KeyCode::Esc, KeyModifiers::NONE), "<escape>");
        assert_eq!(key_to_string(KeyCode::Backspace, KeyModifiers::NONE), "<backspace>");
        assert_eq!(key_to_string(KeyCode::PageUp, KeyModifiers::NONE), "<previous>");
        assert_eq!(key_to_string(KeyCode::PageDown, KeyModifiers::NONE), "<next>");
        assert_eq!(key_to_string(KeyCode::F(1), KeyModifiers::NONE), "<f1>");
        assert_eq!(key_to_string(KeyCode::Left, KeyModifiers::NONE), "<left>");
    }

    #[test]
    fn test_current_keymap() {
        let mut app = test_app();

        app.mode = Mode::Command;
        let km = app.current_keymap().unwrap();
        assert!(km.contains_key("q"));

        app.mode = Mode::Insert;
        let km = app.current_keymap().unwrap();
        assert!(km.contains_key("<enter>"));

        app.mode = Mode::Search;
        let km = app.current_keymap().unwrap();
        assert!(km.contains_key("<escape>"));
    }

    #[test]
    fn test_current_channel() {
        let mut app = test_app();
        assert!(app.current_channel().is_none());

        app.channels.push(ChannelItem::new(
            "C1".to_string(),
            "general".to_string(),
            crate::types::ChannelType::Channel,
        ));
        assert_eq!(app.current_channel().unwrap().name, "general");
    }

    #[test]
    fn test_handle_key_command_mode() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Command;

        handle_key_async(&mut app, KeyCode::Char('i'), KeyModifiers::NONE, &tx);
        assert_eq!(app.mode, Mode::Insert);
    }

    #[test]
    fn test_handle_key_insert_mode_typing() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;

        handle_key_async(&mut app, KeyCode::Char('h'), KeyModifiers::NONE, &tx);
        handle_key_async(&mut app, KeyCode::Char('i'), KeyModifiers::NONE, &tx);
        assert_eq!(app.input, "hi");
    }

    #[test]
    fn test_handle_key_insert_escape() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;

        handle_key_async(&mut app, KeyCode::Esc, KeyModifiers::NONE, &tx);
        assert_eq!(app.mode, Mode::Command);
    }

    #[test]
    fn test_handle_key_search_mode() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Search;

        handle_key_async(&mut app, KeyCode::Char('t'), KeyModifiers::NONE, &tx);
        handle_key_async(&mut app, KeyCode::Char('e'), KeyModifiers::NONE, &tx);
        assert_eq!(app.search_input, "te");

        handle_key_async(&mut app, KeyCode::Esc, KeyModifiers::NONE, &tx);
        assert_eq!(app.mode, Mode::Command);
        assert!(app.search_input.is_empty());
    }

    fn app_with_channels() -> App {
        let mut app = test_app();
        for name in ["general", "random", "dev", "design", "devops"] {
            app.channels.push(ChannelItem::new(
                format!("C-{}", name),
                name.to_string(),
                crate::types::ChannelType::Channel,
            ));
        }
        app
    }

    #[test]
    fn test_channel_search_next_found() {
        let mut app = app_with_channels();
        app.search_input = "dev".to_string();

        let result = app.channel_search_next();
        assert_eq!(result, Some(2)); // "dev" at index 2
        assert_eq!(app.selected_channel, 2);
    }

    #[test]
    fn test_channel_search_next_wraps() {
        let mut app = app_with_channels();
        // channels: general(0), random(1), dev(2), design(3), devops(4)
        app.search_input = "dev".to_string();

        // First match: "dev" at 2
        app.channel_search_next();
        assert_eq!(app.selected_channel, 2);

        // Second match: "design" at 3 (contains "dev" substring? No, "design" does not contain "dev")
        // Actually "design" does NOT contain "dev". Only dev(2) and devops(4) match.
        // Second match: "devops" at 4
        app.channel_search_next();
        assert_eq!(app.selected_channel, 4);

        // Wraps back to "dev" at 2
        app.channel_search_next();
        assert_eq!(app.selected_channel, 2);
    }

    #[test]
    fn test_channel_search_not_found() {
        let mut app = app_with_channels();
        app.search_input = "xyz".to_string();

        let result = app.channel_search_next();
        assert!(result.is_none());
    }

    #[test]
    fn test_channel_search_empty_query() {
        let mut app = app_with_channels();
        app.search_input.clear();

        let result = app.channel_search_next();
        assert!(result.is_none());
    }

    #[test]
    fn test_channel_search_prev() {
        let mut app = app_with_channels();
        app.search_input = "dev".to_string();

        // Set position after devops
        app.last_search_match = Some(4);

        let result = app.channel_search_prev();
        // Should find "devops" at 4 (wrapping backward)
        assert!(result.is_some());
    }

    #[test]
    fn test_channel_search_case_insensitive() {
        let mut app = app_with_channels();
        app.search_input = "DEV".to_string();

        let result = app.channel_search_next();
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_convert_bullets() {
        assert_eq!(convert_bullets("- item one\n- item two"), "• item one\n• item two");
        assert_eq!(convert_bullets("no bullets here"), "no bullets here");
        assert_eq!(convert_bullets("- first\nnot a bullet\n- third"), "• first\nnot a bullet\n• third");
        assert_eq!(convert_bullets("  - indented"), "  • indented");
        assert_eq!(convert_bullets("    - deep"), "    • deep");
    }

    #[test]
    fn test_bullet_prefix() {
        assert_eq!(bullet_prefix("- item"), "- ");
        assert_eq!(bullet_prefix("  - item"), "  - ");
        assert_eq!(bullet_prefix("    - item"), "    - ");
        assert_eq!(bullet_prefix("no bullet"), "");
        assert_eq!(bullet_prefix(""), "");
    }

    #[test]
    fn test_current_line_info() {
        let (start, text) = current_line_info("line1\nline2", 8);
        assert_eq!(start, 6);
        assert_eq!(text, "li");

        let (start, text) = current_line_info("hello", 3);
        assert_eq!(start, 0);
        assert_eq!(text, "hel");
    }

    #[test]
    fn test_newline_continues_bullet() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;
        app.input = "- item one".to_string();
        app.cursor_pos = 10;
        dispatch_action(&mut app, "newline".to_string(), &tx);
        assert_eq!(app.input, "- item one\n- ");
        assert_eq!(app.cursor_pos, 13);
    }

    #[test]
    fn test_newline_continues_indented_bullet() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;
        app.input = "  - sub item".to_string();
        app.cursor_pos = 12;
        dispatch_action(&mut app, "newline".to_string(), &tx);
        assert_eq!(app.input, "  - sub item\n  - ");
        assert_eq!(app.cursor_pos, 17);
    }

    #[test]
    fn test_indent_bullet() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;
        app.input = "- item".to_string();
        app.cursor_pos = 6;
        dispatch_action(&mut app, "indent".to_string(), &tx);
        assert_eq!(app.input, "  - item");
        assert_eq!(app.cursor_pos, 8);
    }

    #[test]
    fn test_dedent_bullet() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;
        app.input = "  - item".to_string();
        app.cursor_pos = 8;
        dispatch_action(&mut app, "dedent".to_string(), &tx);
        assert_eq!(app.input, "- item");
        assert_eq!(app.cursor_pos, 6);
    }

    #[test]
    fn test_indent_no_bullet_noop() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;
        app.input = "plain text".to_string();
        app.cursor_pos = 10;
        dispatch_action(&mut app, "indent".to_string(), &tx);
        assert_eq!(app.input, "plain text");
    }

    #[test]
    fn test_insert_wrap_markers() {
        let mut app = test_app();
        app.input = "hello".to_string();
        app.cursor_pos = 5;
        insert_wrap_markers(&mut app, '*');
        assert_eq!(app.input, "hello**");
        assert_eq!(app.cursor_pos, 6); // between the two *s
    }

    #[test]
    fn test_newline_in_input() {
        let mut app = test_app();
        let tx = test_action_tx();
        app.mode = Mode::Insert;
        app.input = "line1".to_string();
        app.cursor_pos = 5;
        dispatch_action(&mut app, "newline".to_string(), &tx);
        assert_eq!(app.input, "line1\n");
        assert_eq!(app.cursor_pos, 6);
    }

    #[test]
    fn test_key_to_string_shift_enter() {
        assert_eq!(
            key_to_string(KeyCode::Enter, KeyModifiers::SHIFT),
            "<s-enter>"
        );
        assert_eq!(
            key_to_string(KeyCode::Enter, KeyModifiers::NONE),
            "<enter>"
        );
    }
}
