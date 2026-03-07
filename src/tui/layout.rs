use chrono::Datelike;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};
use std::collections::HashMap;

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

/// Render the splash screen. `frame_idx` controls the typewriter animation (0..=total chars).
pub fn render_splash(frame: &mut Frame, frame_idx: usize) {
    let area = frame.area();

    // Pick a deterministic "random" welcome message based on current minute
    let now = chrono::Local::now();
    let msg_idx = (now.timestamp() / 60) as usize % WELCOME_MESSAGES.len();
    let welcome = WELCOME_MESSAGES[msg_idx];

    // Total chars in the logo for the typewriter effect
    let total_logo_chars: usize = LOGO.iter().map(|l| l.len()).sum();

    // Build logo lines with typewriter reveal
    let mut chars_remaining = frame_idx;
    let mut logo_lines: Vec<Line> = Vec::new();
    for &logo_line in LOGO {
        if chars_remaining == 0 {
            break;
        }
        let visible = chars_remaining.min(logo_line.len());
        chars_remaining = chars_remaining.saturating_sub(logo_line.len());
        logo_lines.push(Line::from(Span::styled(
            &logo_line[..visible],
            Style::default().fg(Color::Rgb(100, 200, 140)).add_modifier(Modifier::BOLD),
        )));
    }

    // After logo is fully typed, show welcome message with a fade-in
    let welcome_visible = frame_idx > total_logo_chars;
    let subtitle_line = if welcome_visible {
        let sub_chars = (frame_idx - total_logo_chars).min(welcome.len());
        Line::from(Span::styled(
            &welcome[..sub_chars],
            Style::default().fg(Color::Rgb(180, 180, 220)),
        ))
    } else {
        Line::default()
    };

    // Show "press any key" after everything is typed
    let hint_visible = frame_idx > total_logo_chars + welcome.len();
    let hint_line = if hint_visible {
        Line::from(Span::styled(
            "press any key to continue",
            Style::default().fg(Color::Rgb(90, 90, 110)),
        ))
    } else {
        Line::default()
    };

    // Center vertically: logo height + 2 blank + subtitle + 1 blank + hint
    let content_height = logo_lines.len() + 4;
    let top_pad = area.height.saturating_sub(content_height as u16) / 2;

    let mut lines: Vec<Line> = Vec::new();
    for _ in 0..top_pad {
        lines.push(Line::default());
    }
    lines.extend(logo_lines);
    lines.push(Line::default());
    lines.push(Line::default());
    lines.push(subtitle_line);
    lines.push(Line::default());
    lines.push(hint_line);

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
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

/// Convert cached image pixel rows to Lines for rendering using half-block characters.
fn image_to_lines(rows: &[Vec<(Color, Color)>], sel_bg: Option<Color>) -> Vec<Line<'static>> {
    rows.iter()
        .map(|row| {
            let mut spans: Vec<Span> = Vec::new();
            spans.push(Span::raw("  ")); // indent
            for &(fg, bg) in row {
                let style = Style::default().fg(fg).bg(bg);
                spans.push(Span::styled("\u{2580}", style)); // ▀
            }
            if let Some(bg) = sel_bg {
                // Extend selection background to rest of line
                spans.push(Span::styled(" ", Style::default().bg(bg)));
            }
            Line::from(spans)
        })
        .collect()
}

/// Build display lines for a message with Slack-style layout.
/// `show_header` controls whether the author name + time are shown (false for grouped messages).
fn build_message_lines<'a>(
    msg: &'a crate::types::Message,
    is_selected: bool,
    show_header: bool,
    image_cache: &HashMap<String, Vec<Vec<(Color, Color)>>>,
) -> Vec<Line<'a>> {
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

    // Image files
    for img in &msg.image_files {
        if let Some(rows) = image_cache.get(&img.file_id) {
            result.extend(image_to_lines(rows, sel_bg));
        } else {
            // Show placeholder while loading
            result.push(Line::from(vec![
                Span::styled("  ", style_with_bg(Style::default())),
                Span::styled(
                    format!("[loading image: {}]", img.title),
                    style_with_bg(Style::default().fg(Color::Rgb(100, 100, 130))),
                ),
            ]));
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

    result
}

/// Render chat messages with text wrapping, scroll-to-bottom, selection highlighting,
/// Slack-style grouping, and date separators.
fn render_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let width = area.width as usize;
    let height = area.height as usize;
    if height == 0 || width == 0 {
        return;
    }

    // Build display lines for each message, tracking which msg index each line belongs to.
    // msg_line_ranges[i] = (start_line, end_line) in the flat lines vec.
    let mut all_lines: Vec<Line> = Vec::new();
    let mut msg_line_ranges: Vec<(usize, usize)> = Vec::new();
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
        all_lines.extend(build_message_lines(msg, is_selected, !grouped, &app.image_cache));
        msg_line_ranges.push((start, all_lines.len()));
    }

    let paragraph = Paragraph::new(Text::from(all_lines.clone())).wrap(Wrap { trim: false });
    let line_count = paragraph.line_count(area.width);

    if let Some(sel_idx) = app.selected_message {
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
        let scroll_y = if y_start < app.chat_scroll {
            // Selected message is above viewport — scroll up to show it at top
            y_start
        } else if y_end > app.chat_scroll + height {
            // Selected message is below viewport — scroll down to show it at bottom
            y_end.saturating_sub(height)
        } else {
            // Selected message is visible — don't move
            app.chat_scroll.min(max_scroll)
        };

        // Persist scroll position for next frame
        app.chat_scroll = scroll_y;

        let paragraph = Paragraph::new(Text::from(all_lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll_y as u16, 0));
        frame.render_widget(paragraph, area);
    } else {
        // No selection: scroll to bottom, with chat_scroll as offset from bottom
        let max_scroll = line_count.saturating_sub(height);
        let clamped_scroll = app.chat_scroll.min(max_scroll);
        let scroll_y = max_scroll.saturating_sub(clamped_scroll);

        let paragraph = Paragraph::new(Text::from(all_lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll_y as u16, 0));
        frame.render_widget(paragraph, area);
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
fn render_threads(frame: &mut Frame, app: &App, area: Rect) {
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

    let mut lines: Vec<Line> = Vec::new();
    for (i, msg) in app.thread_messages.iter().enumerate() {
        let grouped = if i > 0 {
            should_group(&app.thread_messages[i - 1], msg)
        } else {
            false
        };
        if !grouped && i > 0 {
            lines.push(Line::default());
        }
        lines.extend(build_message_lines(msg, false, !grouped, &app.image_cache));
    }

    let paragraph = Paragraph::new(Text::from(lines.clone())).wrap(Wrap { trim: false });

    // Scroll to bottom for threads too, clamped to content range
    let line_count = paragraph.line_count(inner.width);
    let max_scroll = line_count.saturating_sub(height);
    let clamped_scroll = app.thread_scroll.min(max_scroll);
    let scroll_y = max_scroll.saturating_sub(clamped_scroll) as u16;

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);
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
        Mode::Command => {
            if app.status.is_empty() {
                Line::from(vec![
                    Span::styled(
                        " COMMAND ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(80, 140, 220))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" i", Style::default().fg(Color::White)),
                    Span::styled("=insert ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("/", Style::default().fg(Color::White)),
                    Span::styled("=search ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("q", Style::default().fg(Color::White)),
                    Span::styled("=quit ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("j/k", Style::default().fg(Color::White)),
                    Span::styled("=nav ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("l/h", Style::default().fg(Color::White)),
                    Span::styled("=focus ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::styled("'", Style::default().fg(Color::White)),
                    Span::styled("=thread", Style::default().fg(Color::Rgb(100, 100, 120))),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        " COMMAND ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(80, 140, 220))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" {}", &app.status)),
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
