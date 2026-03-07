use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use super::App;
use crate::types::Mode;

/// Render the entire application UI.
pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Top-level vertical split: main area + status/input bar at bottom
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(size);

    let main_area = vertical[0];
    let status_area = vertical[1];

    // Horizontal split: sidebar | chat | (optional) threads
    let constraints = if app.thread_visible {
        vec![
            Constraint::Ratio(
                app.config.sidebar_width as u32,
                12,
            ),
            Constraint::Ratio(
                app.config.main_width as u32,
                12,
            ),
            Constraint::Ratio(
                app.config.threads_width as u32,
                12,
            ),
        ]
    } else {
        vec![
            Constraint::Ratio(
                app.config.sidebar_width as u32,
                (app.config.sidebar_width + app.config.main_width) as u32,
            ),
            Constraint::Ratio(
                app.config.main_width as u32,
                (app.config.sidebar_width + app.config.main_width) as u32,
            ),
        ]
    };

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(main_area);

    // Render each pane
    render_channels(frame, app, horizontal[0]);
    render_chat(frame, app, horizontal[1]);

    if app.thread_visible && horizontal.len() > 2 {
        render_threads(frame, app, horizontal[2]);
    }

    // Render status/input bar
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
                crate::types::ChannelType::Channel => "# ",
                crate::types::ChannelType::Group => "* ",
                crate::types::ChannelType::IM => {
                    if ch.presence == "active" {
                        "● "
                    } else {
                        "○ "
                    }
                }
                crate::types::ChannelType::MpIM => "◆ ",
            };

            let style = if i == app.selected_channel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
            } else if ch.notification {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            ListItem::new(format!("{}{}", prefix, ch.display_name())).style(style)
        })
        .collect();

    let title = " Channels ";
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Render the chat/messages pane.
fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let channel_name = app
        .current_channel()
        .map(|ch| ch.display_name().to_string())
        .unwrap_or_else(|| "slackatui".to_string());

    let title = format!(" {} ", channel_name);
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    // Build message lines
    let inner_height = area.height.saturating_sub(2) as usize; // account for borders
    let lines: Vec<Line> = app
        .messages
        .iter()
        .rev()
        .skip(app.chat_scroll)
        .take(inner_height)
        .map(|msg| {
            let time_str = msg.time.format("%H:%M").to_string();
            Line::from(vec![
                Span::styled(
                    format!("{} ", time_str),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}: ", msg.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&msg.content),
            ])
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Render the thread pane.
fn render_threads(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Thread ")
        .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app
        .thread_messages
        .iter()
        .rev()
        .skip(app.thread_scroll)
        .take(inner_height)
        .map(|msg| {
            let time_str = msg.time.format("%H:%M").to_string();
            Line::from(vec![
                Span::styled(
                    format!("{} ", time_str),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}: ", msg.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&msg.content),
            ])
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Render the status/input bar at the bottom.
fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Insert => {
            let cursor_indicator = "▎";
            let (before, after) = app.input.split_at(
                app.cursor_pos.min(app.input.len()),
            );
            Line::from(vec![
                Span::styled("[INSERT] ", Style::default().fg(Color::Green)),
                Span::raw(before),
                Span::styled(cursor_indicator, Style::default().fg(Color::Yellow)),
                Span::raw(after),
            ])
        }
        Mode::Search => {
            Line::from(vec![
                Span::styled("/", Style::default().fg(Color::Yellow)),
                Span::raw(&app.search_input),
                Span::styled("▎", Style::default().fg(Color::Yellow)),
            ])
        }
        Mode::Command => {
            if app.status.is_empty() {
                Line::from(Span::styled(
                    " [COMMAND] q=quit i=insert /=search",
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                Line::from(Span::raw(&app.status))
            }
        }
    };

    let bar = Paragraph::new(content)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
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
