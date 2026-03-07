use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};

use super::App;
use crate::types::{Focus, Mode};

/// Border color for focused vs unfocused panes.
fn border_color(app: &App, pane: Focus) -> Color {
    if app.focus == pane {
        Color::Rgb(100, 200, 140)
    } else {
        Color::Rgb(60, 60, 80)
    }
}

/// Render the entire application UI.
pub fn render(frame: &mut Frame, app: &App) {
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
fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
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
    let input_height = if app.mode == Mode::Insert { 3 } else { 0 };

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

/// Render chat messages with text wrapping, scroll-to-bottom, selection highlighting,
/// and thread reply count indicators.
fn render_messages(frame: &mut Frame, app: &App, area: Rect) {
    let width = area.width as usize;
    let height = area.height as usize;
    if height == 0 || width == 0 {
        return;
    }

    // Build all message lines with their indices
    let mut all_lines: Vec<(usize, Line)> = Vec::new();
    for (i, msg) in app.messages.iter().enumerate() {
        let time_str = msg.time.format("%H:%M").to_string();
        let is_selected = app.selected_message == Some(i);

        // Build the reply indicator
        let reply_indicator = if msg.reply_count > 0 {
            format!(" [{} replies]", msg.reply_count)
        } else if !msg.thread.is_empty() {
            " [thread]".to_string()
        } else {
            String::new()
        };

        let line = Line::from(vec![
            Span::styled(
                format!(" {} ", time_str),
                Style::default().fg(Color::Rgb(100, 100, 120)),
            ),
            Span::styled(
                msg.name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(": ", Style::default().fg(Color::Rgb(100, 100, 120))),
            Span::raw(&msg.content),
            Span::styled(
                reply_indicator,
                Style::default()
                    .fg(Color::Rgb(180, 140, 60))
                    .add_modifier(Modifier::DIM),
            ),
        ]);

        if is_selected {
            // Highlight the entire line for selected message
            let styled_line = Line::from(
                line.spans
                    .into_iter()
                    .map(|s| {
                        Span::styled(
                            s.content,
                            s.style.bg(Color::Rgb(50, 50, 70)),
                        )
                    })
                    .collect::<Vec<_>>(),
            );
            all_lines.push((i, styled_line));
        } else {
            all_lines.push((i, line));
        }
    }

    // If a message is selected, ensure it's visible by computing scroll
    // Otherwise, show newest messages at the bottom (scroll_offset = 0 means bottom)
    let lines: Vec<Line> = all_lines.iter().map(|(_, l)| l.clone()).collect();
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    // Calculate total wrapped height
    let line_count = paragraph.line_count(area.width);

    if let Some(sel_idx) = app.selected_message {
        // Build partial text up to and including selected line to find its y position
        let lines_before: Vec<Line> = all_lines
            .iter()
            .take(sel_idx + 1)
            .map(|(_, l)| l.clone())
            .collect();
        let partial = Paragraph::new(Text::from(lines_before)).wrap(Wrap { trim: false });
        let y_end = partial.line_count(area.width);

        let lines_before_sel: Vec<Line> = all_lines
            .iter()
            .take(sel_idx)
            .map(|(_, l)| l.clone())
            .collect();
        let y_start = if lines_before_sel.is_empty() {
            0
        } else {
            Paragraph::new(Text::from(lines_before_sel))
                .wrap(Wrap { trim: false })
                .line_count(area.width)
        };

        // Determine scroll so selected message is visible
        // We want to keep the view stable, scrolling only if needed
        let scroll_bottom = if line_count > height {
            line_count - height
        } else {
            0
        };

        // Current view: [scroll_y .. scroll_y + height]
        // We need y_start >= scroll_y and y_end <= scroll_y + height
        let scroll_y = if y_start < app.chat_scroll {
            y_start
        } else if y_end > app.chat_scroll + height {
            y_end.saturating_sub(height)
        } else {
            app.chat_scroll.min(scroll_bottom)
        };

        // Store scroll for next frame (can't mutate app here since it's &App)
        // Instead, just use the computed scroll
        let paragraph = Paragraph::new(Text::from(
            all_lines.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>(),
        ))
        .wrap(Wrap { trim: false })
        .scroll((scroll_y as u16, 0));
        frame.render_widget(paragraph, area);
    } else {
        // No selection: scroll to bottom, with chat_scroll as offset from bottom
        let max_scroll = line_count.saturating_sub(height);
        // Clamp chat_scroll to actual content range
        let clamped_scroll = app.chat_scroll.min(max_scroll);
        let scroll_y = max_scroll.saturating_sub(clamped_scroll);

        let paragraph = Paragraph::new(Text::from(
            all_lines.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>(),
        ))
        .wrap(Wrap { trim: false })
        .scroll((scroll_y as u16, 0));
        frame.render_widget(paragraph, area);
    }
}

/// Render the input box inside the chat pane.
fn render_input_box(frame: &mut Frame, app: &App, area: Rect) {
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(80, 80, 110)))
        .border_type(BorderType::Rounded);

    let (before, after) = app
        .input
        .split_at(app.cursor_pos.min(app.input.len()));

    let content = Line::from(vec![
        Span::raw(before),
        Span::styled("\u{258e}", Style::default().fg(Color::Rgb(220, 180, 50))),
        Span::raw(after),
    ]);

    let input = Paragraph::new(content)
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

    let lines: Vec<Line> = app
        .thread_messages
        .iter()
        .map(|msg| {
            let time_str = msg.time.format("%H:%M").to_string();
            Line::from(vec![
                Span::styled(
                    format!(" {} ", time_str),
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                ),
                Span::styled(
                    msg.name.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(": ", Style::default().fg(Color::Rgb(100, 100, 120))),
                Span::raw(&msg.content),
            ])
        })
        .collect();

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    // Scroll to bottom for threads too, clamped to content range
    let line_count = paragraph.line_count(inner.width);
    let max_scroll = line_count.saturating_sub(height);
    let clamped_scroll = app.thread_scroll.min(max_scroll);
    let scroll_y = max_scroll.saturating_sub(clamped_scroll) as u16;

    let paragraph = Paragraph::new(
        app.thread_messages
            .iter()
            .map(|msg| {
                let time_str = msg.time.format("%H:%M").to_string();
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", time_str),
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    ),
                    Span::styled(
                        msg.name.clone(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(": ", Style::default().fg(Color::Rgb(100, 100, 120))),
                    Span::raw(&msg.content),
                ])
            })
            .collect::<Vec<_>>(),
    )
    .wrap(Wrap { trim: false })
    .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);
}

/// Render the status bar at the bottom.
fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Insert => Line::from(vec![
            Span::styled(
                " INSERT ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(100, 200, 140))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " Press Escape to return to command mode",
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
        let app = test_app();
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app);
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
                render(frame, &app);
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
                render(frame, &app);
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
                render(frame, &app);
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
                render(frame, &app);
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
                render(frame, &app);
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
                render(frame, &app);
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
                render(frame, &app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_small_terminal() {
        let app = test_app();
        let backend = ratatui::backend::TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();
    }
}
