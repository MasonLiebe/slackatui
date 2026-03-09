use ratatui::prelude::*;
use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

const CODE_BG: Color = Color::Rgb(40, 44, 52);
const BORDER_FG: Color = Color::Rgb(70, 75, 85);

/// Syntax-highlight a code block and return styled ratatui Lines.
/// Each line is padded to `box_width` and wrapped in │...│ borders.
pub fn highlight_code(code: &str, language: &str, box_width: usize) -> Vec<Line<'static>> {
    let syntax = if language.is_empty() {
        SYNTAX_SET.find_syntax_plain_text()
    } else {
        SYNTAX_SET
            .find_syntax_by_token(language)
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text())
    };

    let theme = &THEME_SET.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);

    // Inner width = box_width - 2 (left "│ ") - 2 (right " │")
    let inner_width = box_width.saturating_sub(4);

    code.lines()
        .map(|line| {
            let ranges = h.highlight_line(line, &SYNTAX_SET).unwrap_or_default();
            let mut spans: Vec<Span> = Vec::new();

            // Left border
            spans.push(Span::styled(
                " \u{2502} ",
                Style::default().fg(BORDER_FG).bg(CODE_BG),
            ));

            // Highlighted code
            let mut char_count = 0;
            for (style, text) in &ranges {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                // Truncate if line exceeds inner width
                let remaining = inner_width.saturating_sub(char_count);
                if remaining == 0 {
                    break;
                }
                let display: String = text.chars().take(remaining).collect();
                char_count += display.chars().count();
                spans.push(Span::styled(
                    display,
                    Style::default().fg(fg).bg(CODE_BG),
                ));
            }

            // Pad to fill inner width
            if char_count < inner_width {
                spans.push(Span::styled(
                    " ".repeat(inner_width - char_count),
                    Style::default().bg(CODE_BG),
                ));
            }

            // Right border
            spans.push(Span::styled(
                " \u{2502}",
                Style::default().fg(BORDER_FG).bg(CODE_BG),
            ));

            Line::from(spans)
        })
        .collect()
}

/// Top border: " ╭─ rust ────────────╮"
pub fn code_block_header(language: &str, box_width: usize) -> Line<'static> {
    let label = if language.is_empty() {
        String::new()
    } else {
        format!(" {} ", language)
    };

    // box_width includes the outer border chars
    // " ╭" = 2, "─...─" fills, "╮" = 1
    let label_len = label.chars().count();
    let fill = box_width.saturating_sub(3 + label_len); // 2 for " ╭" + 1 for "╮"

    let mut spans = Vec::new();
    spans.push(Span::styled(
        " \u{256D}\u{2500}",
        Style::default().fg(BORDER_FG).bg(CODE_BG),
    ));
    if !label.is_empty() {
        spans.push(Span::styled(
            label,
            Style::default()
                .fg(Color::Rgb(150, 160, 180))
                .bg(CODE_BG)
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled(
        "\u{2500}".repeat(fill),
        Style::default().fg(BORDER_FG).bg(CODE_BG),
    ));
    spans.push(Span::styled(
        "\u{256E}",
        Style::default().fg(BORDER_FG).bg(CODE_BG),
    ));

    Line::from(spans)
}

/// Bottom border: " ╰────────────────────╯"
pub fn code_block_footer(box_width: usize) -> Line<'static> {
    // " ╰" = 2, "─...─" fills, "╯" = 1
    let fill = box_width.saturating_sub(3);

    Line::from(vec![
        Span::styled(
            " \u{2570}",
            Style::default().fg(BORDER_FG).bg(CODE_BG),
        ),
        Span::styled(
            "\u{2500}".repeat(fill),
            Style::default().fg(BORDER_FG).bg(CODE_BG),
        ),
        Span::styled(
            "\u{256F}",
            Style::default().fg(BORDER_FG).bg(CODE_BG),
        ),
    ])
}
