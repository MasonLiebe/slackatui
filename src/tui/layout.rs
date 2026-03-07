use chrono::Datelike;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};
use ratatui_image::StatefulImage;

use super::App;
use crate::parse;
use crate::types::{Focus, Mode};

const LOGO: &[&str] = &[
    r"     _            _         _         _ ",
    r"    | |          | |       | |       (_)",
    r" ___| | __ _  ___| | ____ _| |_ _   _ _ ",
    r"/ __| |/ _` |/ __| |/ / _` | __| | | | |",
    r"\__ \ | (_| | (__|   < (_| | |_| |_| | |",
    r"|___/_|\__,_|\___|_|\_\__,_|\__|\__,_|_|",
];

const WELCOME_MESSAGES: &[&str] = &[
    "Your terminal just got chattier.",
    "Slack, but make it retro.",
    "Who needs a browser anyway?",
    "Terminal-grade procrastination.",
    "Now with 100% more monospace.",
    "Because GUIs are overrated.",
    "Alt+Tab? Never heard of her.",
    "Slack from where you code.",
];

/// Splash animation state, created once and mutated each frame.
pub struct SplashState {
    /// Sparkles: (x, y, birth_tick, char_idx)
    sparkles: Vec<(usize, usize, usize, usize)>,
    seed: u64,
    welcome_idx: usize,
}

const SPARKLE_CHARS: &[char] = &['\u{00B7}', '\u{2022}', '\u{2727}'];

impl SplashState {
    pub fn new(width: u16, height: u16) -> Self {
        let now = chrono::Local::now();
        let seed = now.timestamp_millis() as u64;
        let w = width as usize;
        let h = height as usize;
        let mut state = SplashState {
            sparkles: Vec::new(),
            seed,
            welcome_idx: (now.timestamp() / 60) as usize % WELCOME_MESSAGES.len(),
        };

        let num_sparkles = (w * h / 90).min(20);
        for _ in 0..num_sparkles {
            let r = state.next_rand();
            let x = (r % w as u64) as usize;
            let r2 = state.next_rand();
            let y = (r2 % h as u64) as usize;
            let r3 = state.next_rand();
            let char_idx = (r3 % SPARKLE_CHARS.len() as u64) as usize;
            let r4 = state.next_rand();
            let birth = 5 + (r4 % 45) as usize;
            state.sparkles.push((x, y, birth, char_idx));
        }

        state
    }

    fn next_rand(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
}

// Splash animation phases:
//   0..14  - Stem grows upward
//   10..32 - Flower petals bloom outward (with per-petal stagger)
//   18..36 - Logo fades in above
//   36..50 - Welcome message types in
//   48..60 - Hint fades in
//   60+    - Static final frame

/// Render one frame of the splash animation.
pub fn render_splash(frame: &mut Frame, tick: usize, state: &mut SplashState) {
    let area = frame.area();
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h == 0 {
        return;
    }

    let welcome = WELCOME_MESSAGES[state.welcome_idx];

    let logo_w = LOGO.iter().map(|l| l.len()).max().unwrap_or(0);
    let logo_h = LOGO.len();

    // Layout: vertically center the composition
    let flower_radius: f32 = 4.5;
    let stem_h: usize = 4;
    let content_h = logo_h + 2 + (flower_radius as usize) * 2 + 1 + stem_h + 4;
    let top = h.saturating_sub(content_h) / 2;

    let logo_top = top;
    let logo_left = w.saturating_sub(logo_w) / 2;
    let flower_cy = logo_top + logo_h + 2 + flower_radius as usize;
    let flower_cx = w / 2;
    let stem_top = flower_cy + flower_radius as usize + 1;
    let welcome_y = stem_top + stem_h + 1;
    let hint_y = welcome_y + 2;

    let mut chars: Vec<Vec<(char, Style)>> =
        vec![vec![(' ', Style::default()); w]; h];

    // --- Stem grows upward (ticks 2..14) ---
    if tick >= 2 {
        let progress = ((tick as f32 - 2.0) / 12.0).clamp(0.0, 1.0);
        let visible = (progress * stem_h as f32) as usize;
        for i in 0..visible {
            let y = stem_top + stem_h - 1 - i;
            if y < h && flower_cx < w {
                let green = 75 + (i as u8 * 18).min(60);
                chars[y][flower_cx] = (
                    '\u{2502}', // │
                    Style::default().fg(Color::Rgb(40, green, 38)),
                );
            }
        }
        // Tiny leaves
        if progress > 0.55 {
            let ly = stem_top + 1;
            if ly < h && flower_cx >= 1 {
                chars[ly][flower_cx - 1] = (
                    '\u{2572}', // ╲
                    Style::default().fg(Color::Rgb(45, 115, 45)),
                );
            }
        }
        if progress > 0.75 {
            let ly = stem_top;
            if ly < h && flower_cx + 1 < w {
                chars[ly][flower_cx + 1] = (
                    '\u{2571}', // ╱
                    Style::default().fg(Color::Rgb(45, 125, 48)),
                );
            }
        }
    }

    // --- Single flower bloom (ticks 10..32) ---
    if tick >= 10 && flower_cy < h {
        let bloom_base = ((tick as f32 - 10.0) / 22.0).clamp(0.0, 1.0);
        let petal_count = 5.0_f32;
        let pi2 = std::f32::consts::PI * 2.0;
        let ir = flower_radius.ceil() as i32;

        for dy in -ir..=ir {
            for dx in (-ir * 2)..=(ir * 2) {
                let fx = dx as f32 / 2.0; // aspect ratio correction
                let fy = dy as f32;
                let dist = (fx * fx + fy * fy).sqrt();

                if dist < 0.3 || dist > flower_radius {
                    continue;
                }

                let angle = fy.atan2(fx);

                // Petal shape: 5 rounded lobes with gaps between them
                let petal_raw = ((angle * petal_count).cos() * 0.5 + 0.5).powf(0.55);
                let petal_r = flower_radius * (0.3 + petal_raw * 0.7);

                if dist > petal_r {
                    continue;
                }

                // Per-petal stagger: each petal unfurls slightly after the previous
                let petal_idx = ((angle / pi2 * petal_count).rem_euclid(petal_count)) as usize;
                let stagger = petal_idx as f32 * 0.06;
                let bloom = (bloom_base - stagger).clamp(0.0, 1.0);
                let current_r = bloom * petal_r;

                if dist > current_r {
                    continue;
                }

                let x = (flower_cx as i32 + dx) as usize;
                let y = (flower_cy as i32 + dy) as usize;
                if x >= w || y >= h {
                    continue;
                }

                // Soft fade at the bloom edge
                let edge = ((current_r - dist) / 1.2).clamp(0.0, 1.0);

                // Character and color by distance
                let (ch, cr, cg, cb) = if dist < 1.2 {
                    // Warm golden center
                    ('*', 210u8, 180u8, 75u8)
                } else if dist < 2.5 {
                    // Inner petals — soft rose
                    let c = if petal_raw > 0.7 { '*' } else { '\u{00B7}' };
                    (c, 195, 130, 145)
                } else if dist < 3.8 {
                    // Mid petals — dusty pink
                    let c = if petal_raw > 0.6 { '\u{00B7}' } else { '.' };
                    (c, 180, 140, 155)
                } else {
                    // Outer wisps — very faint
                    ('.', 160, 148, 158)
                };

                let r = (cr as f32 * edge) as u8;
                let g = (cg as f32 * edge) as u8;
                let b = (cb as f32 * edge) as u8;

                chars[y][x] = (ch, Style::default().fg(Color::Rgb(r, g, b)));
            }
        }

        // Bright center dot, visible as soon as bloom starts
        if bloom_base > 0.05 && flower_cy < h && flower_cx < w {
            let b = (bloom_base.min(0.4) / 0.4 * 200.0) as u8;
            chars[flower_cy][flower_cx] = (
                '*',
                Style::default()
                    .fg(Color::Rgb(b + 40, (b as f32 * 0.85) as u8 + 30, 50))
                    .add_modifier(Modifier::BOLD),
            );
        }
    }

    // --- Sparkles ---
    for &(sx, sy, birth, ci) in &state.sparkles {
        if tick < birth || sx >= w || sy >= h {
            continue;
        }
        let age = tick - birth;
        let cycle = age % 19;
        if cycle < 4 {
            let brightness = match cycle {
                0 => 35,
                1 => 70,
                2 => 60,
                _ => 30,
            };
            if chars[sy][sx].0 == ' ' {
                chars[sy][sx] = (
                    SPARKLE_CHARS[ci],
                    Style::default().fg(Color::Rgb(
                        brightness + 35,
                        brightness + 35,
                        brightness + 55,
                    )),
                );
            }
        }
    }

    // --- Logo (tick 18+) ---
    if tick >= 18 {
        let logo_progress = ((tick - 18) as f32 / 18.0).min(1.0);

        for (li, &logo_line) in LOGO.iter().enumerate() {
            let y = logo_top + li;
            if y >= h {
                break;
            }
            for (ci, ch) in logo_line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = logo_left + ci;
                if x >= w {
                    break;
                }

                // Bloom from center outward
                if logo_progress < 1.0 {
                    let center = logo_w as f32 / 2.0;
                    let dist = ((ci as f32 - center).abs() / center
                        + (li as f32 / logo_h as f32) * 0.3)
                        / 1.3;
                    if dist > logo_progress {
                        continue;
                    }
                }

                let settle = ((tick as f32 - 18.0) / 30.0).min(1.0);
                let wave = if tick >= 36 {
                    let w = (tick as f32 * 0.1 - ci as f32 * 0.05).sin() * 0.5 + 0.5;
                    w * (1.0 - settle).max(0.12)
                } else {
                    0.4
                };
                let r = (170.0 - settle * 70.0 + wave * 35.0) as u8;
                let g = (155.0 + settle * 45.0 + wave * 25.0) as u8;
                let b = (80.0 + settle * 60.0 + wave * 35.0) as u8;

                chars[y][x] = (
                    ch,
                    Style::default()
                        .fg(Color::Rgb(r, g, b))
                        .add_modifier(Modifier::BOLD),
                );
            }
        }
    }

    // --- Welcome message (tick 36+) ---
    if tick >= 36 {
        if welcome_y < h {
            let progress = ((tick - 36) as f32 / 14.0).min(1.0);
            let visible = (progress * welcome.len() as f32) as usize;
            let left = w.saturating_sub(welcome.len()) / 2;
            for (ci, ch) in welcome.chars().take(visible).enumerate() {
                let x = left + ci;
                if x < w {
                    let glow = if ci + 2 >= visible { 200 } else { 165 };
                    chars[welcome_y][x] = (
                        ch,
                        Style::default().fg(Color::Rgb(glow, glow - 25, glow + 10)),
                    );
                }
            }
        }
    }

    // --- Hint (tick 48+) ---
    if tick >= 48 {
        let hint = "press any key to continue";
        if hint_y < h {
            let fade = (((tick - 48) as f32 / 12.0).min(1.0) * 110.0) as u8;
            let left = w.saturating_sub(hint.len()) / 2;
            for (ci, ch) in hint.chars().enumerate() {
                let x = left + ci;
                if x < w {
                    chars[hint_y][x] = (
                        ch,
                        Style::default().fg(Color::Rgb(fade, fade - 8, fade + 18)),
                    );
                }
            }
        }
    }

    // Convert buffer to Lines
    let lines: Vec<Line> = chars
        .iter()
        .map(|row| {
            let mut spans: Vec<Span> = Vec::new();
            let mut current_text = String::new();
            let mut current_style = Style::default();
            for &(ch, style) in row {
                if style == current_style {
                    current_text.push(ch);
                } else {
                    if !current_text.is_empty() {
                        spans.push(Span::styled(
                            std::mem::take(&mut current_text),
                            current_style,
                        ));
                    }
                    current_text.push(ch);
                    current_style = style;
                }
            }
            if !current_text.is_empty() {
                spans.push(Span::styled(current_text, current_style));
            }
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Border color for focused vs unfocused panes.
fn border_color(app: &App, pane: Focus) -> Color {
    if app.focus == pane {
        Color::Rgb(100, 200, 140)
    } else {
        Color::Rgb(60, 60, 80)
    }
}

/// Convert message content into styled spans using mrkdwn parsing.
fn content_spans(content: &str) -> Vec<Span<'_>> {
    let segments = parse::parse_mrkdwn(content);
    segments
        .into_iter()
        .map(|seg| {
            let mut style = Style::default();
            if seg.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if seg.italic {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if seg.strikethrough {
                style = style.add_modifier(Modifier::CROSSED_OUT);
            }
            if seg.code {
                style = style.fg(Color::Rgb(220, 170, 80)).bg(Color::Rgb(40, 40, 50));
            }
            Span::styled(seg.text, style)
        })
        .collect()
}

/// Render the entire application UI.
pub fn render(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Vertical: main area + 1-line status bar
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(size);

    let main_area = vertical[0];
    let status_area = vertical[1];

    // Sidebar uses fixed character width for readability
    let sidebar_chars = 16 + (app.config.sidebar_width as u16) * 4;

    let constraints = if app.thread_visible {
        let thread_chars = 16 + (app.config.threads_width as u16) * 4;
        vec![
            Constraint::Length(sidebar_chars),
            Constraint::Min(20),
            Constraint::Length(thread_chars),
        ]
    } else {
        vec![Constraint::Length(sidebar_chars), Constraint::Min(20)]
    };

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(main_area);

    render_channels(frame, app, horizontal[0]);
    render_chat(frame, app, horizontal[1]);

    if app.thread_visible && horizontal.len() > 2 {
        render_threads(frame, app, horizontal[2]);
    }

    render_status(frame, app, status_area);

    // Reaction picker overlay
    if app.mode == Mode::React {
        render_react_picker(frame, app, size);
    }
}

/// Render the channel sidebar.
fn render_channels(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .channels
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            let prefix = match ch.channel_type {
                crate::types::ChannelType::Channel => "  # ",
                crate::types::ChannelType::Group => "  * ",
                crate::types::ChannelType::IM => {
                    if ch.presence == "active" {
                        "  ● "
                    } else {
                        "  ○ "
                    }
                }
                crate::types::ChannelType::MpIM => "  ◆ ",
            };

            let style = if i == app.selected_channel {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(50, 50, 70))
                    .add_modifier(Modifier::BOLD)
            } else if ch.notification {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(140, 140, 160))
            };

            ListItem::new(format!("{}{}", prefix, ch.display_name())).style(style)
        })
        .collect();

    let bc = border_color(app, Focus::Channels);
    let block = Block::default()
        .title(" Channels ")
        .title_style(
            Style::default()
                .fg(Color::Rgb(100, 200, 140))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(bc))
        .border_type(BorderType::Rounded);

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Render the chat/messages pane with embedded input box.
fn render_chat(frame: &mut Frame, app: &mut App, area: Rect) {
    let channel_name = app
        .current_channel()
        .map(|ch| ch.display_name().to_string())
        .unwrap_or_else(|| "slackatui".to_string());

    let bc = border_color(app, Focus::Chat);
    let block = Block::default()
        .title(format!(" {} ", channel_name))
        .title_style(
            Style::default()
                .fg(Color::Rgb(100, 200, 140))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(bc))
        .border_type(BorderType::Rounded);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner: messages area + input box (when inserting)
    // Height grows with newlines, min 3 (border + 1 line), max 10
    let input_height = if app.mode == Mode::Insert {
        let line_count = app.input.chars().filter(|&c| c == '\n').count() + 1;
        (line_count as u16 + 2).min(10).max(3)
    } else {
        0
    };

    let chat_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(input_height)])
        .split(inner);

    let msg_area = chat_split[0];

    render_messages(frame, app, msg_area);

    if app.mode == Mode::Insert {
        render_input_box(frame, app, chat_split[1]);
    }
}

/// Determine if two consecutive messages should be grouped (same author, within 5 minutes).
fn should_group(prev: &crate::types::Message, curr: &crate::types::Message) -> bool {
    prev.name == curr.name
        && prev.time.date_naive() == curr.time.date_naive()
        && (curr.time - prev.time).num_minutes().abs() < 5
}

/// Build a centered date separator line like "── Thursday, March 5th ──".
fn date_separator(date: chrono::NaiveDate) -> Line<'static> {
    let formatted = date.format("%A, %B %-d").to_string();
    // Add ordinal suffix
    let day = date.day();
    let suffix = match day {
        1 | 21 | 31 => "st",
        2 | 22 => "nd",
        3 | 23 => "rd",
        _ => "th",
    };
    let label = format!(" {}{} ", formatted, suffix);
    Line::from(vec![
        Span::styled("────", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            label,
            Style::default()
                .fg(Color::Rgb(160, 160, 180))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("────", Style::default().fg(Color::Rgb(60, 60, 80))),
    ])
    .alignment(Alignment::Center)
}

/// Height in terminal rows reserved for an inline image.
const IMAGE_ROWS: u16 = 15;

/// Build display lines for a message with Slack-style layout.
/// `show_header` controls whether the author name + time are shown (false for grouped messages).
/// Returns (lines, image_placeholders) where each placeholder is (line_offset_within_result, file_id).
fn build_message_lines<'a>(
    msg: &'a crate::types::Message,
    is_selected: bool,
    show_header: bool,
    image_cache_keys: &std::collections::HashSet<String>,
) -> (Vec<Line<'a>>, Vec<(usize, String)>) {
    let sel_bg = if is_selected {
        Some(Color::Rgb(50, 50, 70))
    } else {
        None
    };

    let style_with_bg = |s: Style| -> Style {
        if let Some(bg) = sel_bg {
            s.bg(bg)
        } else {
            s
        }
    };

    let mut result = Vec::new();

    if show_header {
        // Header line: "  Name  10:30 AM"
        let time_str = msg.time.format("%-I:%M %p").to_string();
        result.push(Line::from(vec![
            Span::styled("  ", style_with_bg(Style::default())),
            Span::styled(
                msg.name.clone(),
                style_with_bg(
                    Style::default()
                        .fg(Color::Rgb(220, 220, 240))
                        .add_modifier(Modifier::BOLD),
                ),
            ),
            Span::styled(
                format!("  {}", time_str),
                style_with_bg(Style::default().fg(Color::Rgb(100, 100, 120))),
            ),
        ]));
    }

    // Content lines, indented
    let content_lines: Vec<&str> = msg.content.split('\n').collect();
    for (line_idx, content_line) in content_lines.iter().enumerate() {
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled("  ", style_with_bg(Style::default())));

        for seg_span in content_spans(content_line) {
            spans.push(Span::styled(
                seg_span.content,
                style_with_bg(seg_span.style),
            ));
        }

        // Reply indicator on the last content line
        if line_idx == content_lines.len() - 1 {
            if msg.reply_count > 0 {
                let reply_text = if msg.reply_count == 1 {
                    " ↳ 1 reply".to_string()
                } else {
                    format!(" ↳ {} replies", msg.reply_count)
                };
                spans.push(Span::styled(
                    reply_text,
                    style_with_bg(
                        Style::default()
                            .fg(Color::Rgb(80, 160, 220))
                            .add_modifier(Modifier::BOLD),
                    ),
                ));
            } else if !msg.thread.is_empty() {
                spans.push(Span::styled(
                    " ↳ thread",
                    style_with_bg(Style::default().fg(Color::Rgb(100, 100, 130))),
                ));
            }
        }

        result.push(Line::from(spans));
    }

    // Inline images: insert placeholder blank lines for cached images
    let mut image_placeholders = Vec::new();
    for file in &msg.files {
        if file.is_image && image_cache_keys.contains(&file.file_id) {
            let offset = result.len();
            image_placeholders.push((offset, file.file_id.clone()));
            for _ in 0..IMAGE_ROWS {
                result.push(Line::default());
            }
        }
    }

    // Reactions line
    if !msg.reactions.is_empty() {
        let mut reaction_spans: Vec<Span> = Vec::new();
        reaction_spans.push(Span::styled("  ", style_with_bg(Style::default())));
        for (i, reaction) in msg.reactions.iter().enumerate() {
            if i > 0 {
                reaction_spans.push(Span::styled(" ", style_with_bg(Style::default())));
            }
            let label = format!(" {} {} ", reaction.emoji, reaction.count);
            let style = if reaction.reacted {
                Style::default()
                    .fg(Color::Rgb(80, 160, 220))
                    .bg(Color::Rgb(30, 50, 70))
            } else {
                Style::default()
                    .fg(Color::Rgb(160, 160, 180))
                    .bg(Color::Rgb(45, 45, 60))
            };
            reaction_spans.push(Span::styled(label, style_with_bg(style)));
        }
        result.push(Line::from(reaction_spans));
    }

    (result, image_placeholders)
}

/// Render chat messages with text wrapping, scroll-to-bottom, selection highlighting,
/// Slack-style grouping, and date separators.
fn render_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let width = area.width as usize;
    let height = area.height as usize;
    if height == 0 || width == 0 {
        return;
    }

    // Collect cached file IDs for build_message_lines
    let cached_keys: std::collections::HashSet<String> =
        app.image_cache.keys().cloned().collect();

    // Build display lines for each message, tracking which msg index each line belongs to.
    // msg_line_ranges[i] = (start_line, end_line) in the flat lines vec.
    // image_positions: (absolute_line, file_id) for images to render after the paragraph.
    let mut all_lines: Vec<Line> = Vec::new();
    let mut msg_line_ranges: Vec<(usize, usize)> = Vec::new();
    let mut image_positions: Vec<(usize, String)> = Vec::new();
    let mut last_date: Option<chrono::NaiveDate> = None;

    for (i, msg) in app.messages.iter().enumerate() {
        let is_selected = app.selected_message == Some(i);

        // Date separator if the day changed
        let msg_date = msg.time.date_naive();
        if last_date != Some(msg_date) {
            if last_date.is_some() {
                all_lines.push(Line::default()); // spacing before separator
            }
            all_lines.push(date_separator(msg_date));
            all_lines.push(Line::default()); // spacing after separator
            last_date = Some(msg_date);
        }

        // Determine if this message should be grouped with the previous one
        let grouped = if i > 0 {
            should_group(&app.messages[i - 1], msg)
        } else {
            false
        };

        // Add a blank line before new message groups (not between grouped messages)
        if !grouped && i > 0 {
            all_lines.push(Line::default());
        }

        let start = all_lines.len();
        let (lines, placeholders) = build_message_lines(msg, is_selected, !grouped, &cached_keys);
        // Map relative placeholder offsets to absolute line positions
        for (offset, file_id) in placeholders {
            image_positions.push((start + offset, file_id));
        }
        all_lines.extend(lines);
        msg_line_ranges.push((start, all_lines.len()));
    }

    let paragraph = Paragraph::new(Text::from(all_lines.clone())).wrap(Wrap { trim: false });
    let line_count = paragraph.line_count(area.width);

    let scroll_y = if let Some(sel_idx) = app.selected_message {
        // Compute wrapped y positions for the selected message
        let (sel_start_line, sel_end_line) = msg_line_ranges
            .get(sel_idx)
            .copied()
            .unwrap_or((0, 0));

        let y_start = if sel_start_line == 0 {
            0
        } else {
            Paragraph::new(Text::from(all_lines[..sel_start_line].to_vec()))
                .wrap(Wrap { trim: false })
                .line_count(area.width)
        };
        let y_end = Paragraph::new(Text::from(all_lines[..sel_end_line].to_vec()))
            .wrap(Wrap { trim: false })
            .line_count(area.width);

        let max_scroll = line_count.saturating_sub(height);

        // Keep view stable: only scroll when selected message goes off-screen
        let sy = if y_start < app.chat_scroll {
            y_start
        } else if y_end > app.chat_scroll + height {
            y_end.saturating_sub(height)
        } else {
            app.chat_scroll.min(max_scroll)
        };
        app.chat_scroll = sy;
        sy
    } else {
        // No selection: scroll to bottom, with chat_scroll as offset from bottom
        let max_scroll = line_count.saturating_sub(height);
        let clamped_scroll = app.chat_scroll.min(max_scroll);
        max_scroll.saturating_sub(clamped_scroll)
    };

    let paragraph = Paragraph::new(Text::from(all_lines.clone()))
        .wrap(Wrap { trim: false })
        .scroll((scroll_y as u16, 0));
    frame.render_widget(paragraph, area);

    // Render images at their placeholder positions using StatefulImage
    for (abs_line, file_id) in &image_positions {
        // Compute wrapped y for this placeholder line
        let img_y = if *abs_line == 0 {
            0
        } else {
            Paragraph::new(Text::from(all_lines[..*abs_line].to_vec()))
                .wrap(Wrap { trim: false })
                .line_count(area.width)
        };
        // Position relative to viewport
        let rel_y = img_y as i32 - scroll_y as i32;
        if rel_y < 0 || rel_y >= height as i32 {
            continue; // off-screen
        }
        let visible_rows = (height as i32 - rel_y).min(IMAGE_ROWS as i32) as u16;
        if visible_rows == 0 {
            continue;
        }
        let img_area = Rect::new(
            area.x + 2, // indent
            area.y + rel_y as u16,
            area.width.saturating_sub(4),
            visible_rows,
        );
        if let Some(protocol) = app.image_cache.get_mut(file_id) {
            let image_widget = StatefulImage::default();
            frame.render_stateful_widget(image_widget, img_area, protocol);
        }
    }
}

/// Render the input box inside the chat pane.
fn render_input_box(frame: &mut Frame, app: &App, area: Rect) {
    let title = if app.reply_thread_ts.is_some() {
        " reply "
    } else {
        ""
    };
    let border_color = if app.reply_thread_ts.is_some() {
        Color::Rgb(180, 140, 60)
    } else {
        Color::Rgb(80, 80, 110)
    };
    let input_block = Block::default()
        .title(title)
        .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .border_type(BorderType::Rounded);

    let pos = app.cursor_pos.min(app.input.len());
    let (before, after) = app.input.split_at(pos);

    // Split into lines, placing the cursor marker at the split point
    let before_lines: Vec<&str> = before.split('\n').collect();
    let after_lines: Vec<&str> = after.split('\n').collect();

    let cursor_line_idx = before_lines.len() - 1;
    let cursor_span = Span::styled("\u{258e}", Style::default().fg(Color::Rgb(220, 180, 50)));

    let mut lines: Vec<Line> = Vec::new();

    // Lines before the cursor line
    for &line in &before_lines[..cursor_line_idx] {
        lines.push(Line::from(Span::raw(line.to_string())));
    }

    // The cursor line: end of before + cursor + start of after
    let cursor_line_before = before_lines[cursor_line_idx];
    let cursor_line_after = after_lines[0];
    lines.push(Line::from(vec![
        Span::raw(cursor_line_before.to_string()),
        cursor_span,
        Span::raw(cursor_line_after.to_string()),
    ]));

    // Lines after the cursor line
    for &line in &after_lines[1..] {
        lines.push(Line::from(Span::raw(line.to_string())));
    }

    let input = Paragraph::new(lines)
        .block(input_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(input, area);
}

/// Render the thread pane.
fn render_threads(frame: &mut Frame, app: &mut App, area: Rect) {
    let bc = border_color(app, Focus::Thread);
    let block = Block::default()
        .title(" Thread ")
        .title_style(
            Style::default()
                .fg(Color::Rgb(100, 200, 140))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(bc))
        .border_type(BorderType::Rounded);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let height = inner.height as usize;
    if height == 0 {
        return;
    }

    let cached_keys: std::collections::HashSet<String> =
        app.image_cache.keys().cloned().collect();

    let mut lines: Vec<Line> = Vec::new();
    let mut image_positions: Vec<(usize, String)> = Vec::new();
    for (i, msg) in app.thread_messages.iter().enumerate() {
        let grouped = if i > 0 {
            should_group(&app.thread_messages[i - 1], msg)
        } else {
            false
        };
        if !grouped && i > 0 {
            lines.push(Line::default());
        }
        let start = lines.len();
        let (msg_lines, placeholders) = build_message_lines(msg, false, !grouped, &cached_keys);
        for (offset, file_id) in placeholders {
            image_positions.push((start + offset, file_id));
        }
        lines.extend(msg_lines);
    }

    let paragraph = Paragraph::new(Text::from(lines.clone())).wrap(Wrap { trim: false });

    // Scroll to bottom for threads too, clamped to content range
    let line_count = paragraph.line_count(inner.width);
    let max_scroll = line_count.saturating_sub(height);
    let clamped_scroll = app.thread_scroll.min(max_scroll);
    let scroll_y = max_scroll.saturating_sub(clamped_scroll) as u16;

    let paragraph = Paragraph::new(Text::from(lines.clone()))
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);

    // Render images at their placeholder positions
    for (abs_line, file_id) in &image_positions {
        let img_y = if *abs_line == 0 {
            0
        } else {
            Paragraph::new(Text::from(lines[..*abs_line].to_vec()))
                .wrap(Wrap { trim: false })
                .line_count(inner.width)
        };
        let rel_y = img_y as i32 - scroll_y as i32;
        if rel_y < 0 || rel_y >= height as i32 {
            continue;
        }
        let visible_rows = (height as i32 - rel_y).min(IMAGE_ROWS as i32) as u16;
        if visible_rows == 0 {
            continue;
        }
        let img_area = Rect::new(
            inner.x + 2,
            inner.y + rel_y as u16,
            inner.width.saturating_sub(4),
            visible_rows,
        );
        if let Some(protocol) = app.image_cache.get_mut(file_id) {
            let image_widget = StatefulImage::default();
            frame.render_stateful_widget(image_widget, img_area, protocol);
        }
    }
}

/// Render the status bar at the bottom.
fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Insert => {
            let mut spans = vec![
                Span::styled(
                    " INSERT ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(100, 200, 140))
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            if !app.status.is_empty() && app.status != "-- INSERT --" {
                spans.push(Span::styled(
                    format!(" {}", &app.status),
                    Style::default().fg(Color::Rgb(180, 140, 60)),
                ));
            } else {
                spans.push(Span::styled(
                    " Escape to exit",
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                ));
            }
            Line::from(spans)
        }
        Mode::React => Line::from(vec![
            Span::styled(
                " REACT ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(220, 180, 50))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " type to search, Enter to select, Esc to cancel",
                Style::default().fg(Color::Rgb(100, 100, 120)),
            ),
        ]),
        Mode::Search => Line::from(vec![
            Span::styled(
                " SEARCH ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(220, 180, 50))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" /", Style::default().fg(Color::Rgb(220, 180, 50))),
            Span::raw(&app.search_input),
            Span::styled("\u{258e}", Style::default().fg(Color::Rgb(220, 180, 50))),
        ]),
        Mode::Upload => Line::from(vec![
            Span::styled(
                " UPLOAD ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(180, 120, 220))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" file: ", Style::default().fg(Color::Rgb(180, 120, 220))),
            Span::raw(&app.upload_path),
            Span::styled("\u{258e}", Style::default().fg(Color::Rgb(180, 120, 220))),
            Span::styled(
                "  Enter=upload, Esc=cancel",
                Style::default().fg(Color::Rgb(100, 100, 120)),
            ),
        ]),
        Mode::Command => {
            let mode_badge = Span::styled(
                " COMMAND ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(80, 140, 220))
                    .add_modifier(Modifier::BOLD),
            );
            if !app.staged_files.is_empty() {
                let names: Vec<&str> = app
                    .staged_files
                    .iter()
                    .map(|p| {
                        std::path::Path::new(p)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(p)
                    })
                    .collect();
                let count = app.staged_files.len();
                Line::from(vec![
                    mode_badge,
                    Span::styled(
                        format!(
                            " {} file{} staged: {}",
                            count,
                            if count == 1 { "" } else { "s" },
                            names.join(", ")
                        ),
                        Style::default().fg(Color::Rgb(180, 120, 220)),
                    ),
                    Span::styled(" Enter", Style::default().fg(Color::White)),
                    Span::styled("=upload ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("x", Style::default().fg(Color::White)),
                    Span::styled("=clear", Style::default().fg(Color::Rgb(100, 100, 120))),
                ])
            } else if !app.status.is_empty() {
                Line::from(vec![
                    mode_badge,
                    Span::raw(format!(" {}", &app.status)),
                ])
            } else if app.focus == Focus::Chat && app.selected_message.is_some() {
                // Contextual hints for selected message
                let msg = app.selected_message.and_then(|i| app.messages.get(i));
                let has_file = msg.map_or(false, |m| !m.files.is_empty());
                let has_replies = msg.map_or(false, |m| m.reply_count > 0 || !m.thread.is_empty());
                let mut spans = vec![mode_badge];
                spans.push(Span::styled(" i", Style::default().fg(Color::White)));
                spans.push(Span::styled("=insert ", Style::default().fg(Color::Rgb(100, 100, 120))));
                spans.push(Span::styled("r", Style::default().fg(Color::White)));
                spans.push(Span::styled("=reply ", Style::default().fg(Color::Rgb(100, 100, 120))));
                spans.push(Span::styled("e", Style::default().fg(Color::White)));
                spans.push(Span::styled("=react ", Style::default().fg(Color::Rgb(100, 100, 120))));
                if has_replies {
                    spans.push(Span::styled("'", Style::default().fg(Color::White)));
                    spans.push(Span::styled("=thread ", Style::default().fg(Color::Rgb(100, 100, 120))));
                }
                if has_file {
                    spans.push(Span::styled("o", Style::default().fg(Color::Rgb(100, 200, 140))));
                    spans.push(Span::styled("=open file ", Style::default().fg(Color::Rgb(100, 200, 140))));
                }
                spans.push(Span::styled("u", Style::default().fg(Color::White)));
                spans.push(Span::styled("=upload ", Style::default().fg(Color::Rgb(100, 100, 120))));
                spans.push(Span::styled("j/k", Style::default().fg(Color::White)));
                spans.push(Span::styled("=nav ", Style::default().fg(Color::Rgb(100, 100, 120))));
                spans.push(Span::styled("h", Style::default().fg(Color::White)));
                spans.push(Span::styled("=back", Style::default().fg(Color::Rgb(100, 100, 120))));
                Line::from(spans)
            } else {
                Line::from(vec![
                    mode_badge,
                    Span::styled(" i", Style::default().fg(Color::White)),
                    Span::styled("=insert ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("/", Style::default().fg(Color::White)),
                    Span::styled("=search ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("j/k", Style::default().fg(Color::White)),
                    Span::styled("=nav ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("l/h", Style::default().fg(Color::White)),
                    Span::styled("=focus ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("'", Style::default().fg(Color::White)),
                    Span::styled("=thread ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("q", Style::default().fg(Color::White)),
                    Span::styled("=quit", Style::default().fg(Color::Rgb(100, 100, 120))),
                ])
            }
        }
    };

    let bar = Paragraph::new(content)
        .style(Style::default().bg(Color::Rgb(30, 30, 40)).fg(Color::White));
    frame.render_widget(bar, area);
}

/// Render the emoji reaction picker as a centered popup.
fn render_react_picker(frame: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::Clear;

    let popup_width = 40u16.min(area.width.saturating_sub(4));
    let popup_height = 14u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" React ")
        .title_style(
            Style::default()
                .fg(Color::Rgb(220, 180, 50))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(220, 180, 50)))
        .border_type(BorderType::Rounded);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 2 || inner.width < 5 {
        return;
    }

    // Search input line
    let search_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let results_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height.saturating_sub(1));

    let search_line = Line::from(vec![
        Span::styled(" /", Style::default().fg(Color::Rgb(220, 180, 50))),
        Span::raw(&app.react_query),
        Span::styled("\u{258e}", Style::default().fg(Color::Rgb(220, 180, 50))),
    ]);
    frame.render_widget(Paragraph::new(search_line), search_area);

    // Emoji results
    let max_visible = results_area.height as usize;
    let scroll = if app.react_selected >= max_visible {
        app.react_selected - max_visible + 1
    } else {
        0
    };

    let mut lines: Vec<Line> = Vec::new();
    for (i, (name, emoji)) in app.react_results.iter().enumerate().skip(scroll).take(max_visible) {
        let is_sel = i == app.react_selected;
        let style = if is_sel {
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(60, 60, 90))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(180, 180, 200))
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {}  ", emoji), style),
            Span::styled(format!(":{}: ", name), style),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), results_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::types::{ChannelItem, ChannelType};

    fn test_app() -> App {
        App::new(Config::default())
    }

    #[test]
    fn test_render_does_not_panic_empty() {
        let mut app = test_app();
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_with_channels() {
        let mut app = test_app();
        app.channels.push(ChannelItem::new(
            "C1".into(),
            "general".into(),
            ChannelType::Channel,
        ));
        app.channels.push(ChannelItem::new(
            "C2".into(),
            "random".into(),
            ChannelType::Channel,
        ));

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_with_messages() {
        let mut app = test_app();
        app.messages.push(crate::types::Message::new(
            "1.0".into(),
            "alice".into(),
            "hello".into(),
            chrono::Local::now(),
        ));

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_with_selected_message() {
        let mut app = test_app();
        app.messages.push(crate::types::Message::new(
            "1.0".into(),
            "alice".into(),
            "hello".into(),
            chrono::Local::now(),
        ));
        app.selected_message = Some(0);
        app.focus = Focus::Chat;

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_with_reply_count() {
        let mut app = test_app();
        let mut msg = crate::types::Message::new(
            "1.0".into(),
            "alice".into(),
            "hello".into(),
            chrono::Local::now(),
        );
        msg.reply_count = 5;
        app.messages.push(msg);

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_with_threads_visible() {
        let mut app = test_app();
        app.thread_visible = true;
        app.thread_messages.push(crate::types::Message::new(
            "2.0".into(),
            "bob".into(),
            "reply".into(),
            chrono::Local::now(),
        ));

        let backend = ratatui::backend::TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_insert_mode() {
        let mut app = test_app();
        app.mode = Mode::Insert;
        app.input = "hello".to_string();
        app.cursor_pos = 5;

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_search_mode() {
        let mut app = test_app();
        app.mode = Mode::Search;
        app.search_input = "test".to_string();

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_small_terminal() {
        let mut app = test_app();
        let backend = ratatui::backend::TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &mut app);
            })
            .unwrap();
    }
}
